use rppal::gpio::{Gpio, OutputPin};
use rppal::i2c::I2c;
use std::cmp::min;
use std::fmt::{self, write};
use std::io::Error;
use std::result::Result::Err;
use std::thread::sleep;
use std::time::Duration;

// --- --- --- --- CCS811 Constants --- --- --- ---
pub enum Ccs811Mode {
    Idle = 0,
    Sec1 = 1,
    Sec10 = 2,
    Sec60 = 3,
}

pub const CCS811_SLAVEADDR_0: u16 = 0x5A;
pub const CCS811_SLAVEADDR_1: u16 = 0x5B;

// CCS811 registers/mailboxes, all 1 byte except when stated otherwise
pub const CCS811_STATUS: u8 = 0x00;
pub const CCS811_ERR: u8 = 0xE0;
pub const CCS811_MEAS_MODE: u8 = 0x01;
pub const CCS811_ALG_RESULT_DATA: u8 = 0x02; // up to 8 bytes
pub const CCS811_ENV_DATA: u8 = 0x05; // 4 bytes
pub const CCS811_BASELINE: u8 = 0x11; // 2 bytes
pub const CCS811_HW_ID: u8 = 0x20;
pub const CCS811_HW_VERSION: u8 = 0x21;
pub const CCS811_FW_BOOT_VERSION: u8 = 0x23; // 2 bytes
pub const CCS811_FW_APP_VERSION: u8 = 0x24; // 2 bytes
pub const CCS811_APP_ERASE: u8 = 0xF1; // 4 bytes
pub const CCS811_APP_DATA: u8 = 0xF2; // 9 bytes
pub const CCS811_APP_VERIFY: u8 = 0xF3; // 0 bytes
pub const CCS811_APP_START: u8 = 0xF4; // 0 bytes
pub const CCS811_SW_RESET: u8 = 0xFF; // 4 bytes

pub const CCS811_STATUS_APP_MODE: u8 = 0b10000000; // Else boot mode
pub const CCS811_STATUS_APP_ERASE: u8 = 0b01000000; // Else no erase completed
pub const CCS811_STATUS_APP_VERIFY: u8 = 0b00100000; // Else no verify completed
pub const CCS811_STATUS_APP_VALID: u8 = 0b00010000; // Else no valid app firmware loaded

pub const CCS811_WAIT_AFTER_RESET_US: Duration = Duration::from_micros(2000); // The CCS811 needs a wait after reset
pub const CCS811_WAIT_AFTER_APPSTART_US: Duration = Duration::from_micros(1000); // The CCS811 needs a wait after app start
pub const CCS811_WAIT_AFTER_WAKE_US: Duration = Duration::from_micros(50); // The CCS811 needs a wait after WAKE signal
pub const CCS811_WAIT_AFTER_APPERASE_MS: Duration = Duration::from_millis(500); // The CCS811 needs a wait after app erase (300ms from spec not enough)
pub const CCS811_WAIT_AFTER_APPVERIFY_MS: Duration = Duration::from_millis(70); // The CCS811 needs a wait after app verify
pub const CCS811_WAIT_AFTER_APPDATA_MS: Duration = Duration::from_millis(50); // The CCS811 needs a wait after writing app data

/// Bytes are calculated by taking the value without fraction and put it's 7 bits to the first byte.
/// The fraction is multiplied by 512 as described in the CCS811 specs. To ensure
/// The value can not be higher than 127 but humidity and temperature, this function is used for, will never
/// exceed this.
fn float_to_bytes(value: f32) -> [u8; 2] {
    let base = value.floor();
    // We only have 9 bits. 512 are already 10. So we ensure with min() that max 511 is used for fraction
    let fraction = min(((value - base) * 512.0 - 1.0) as u16, 511);
    // Take 7 bits of base and 1 bit of fraction
    let hi = ((base as u8 & 0b1111111) << 1) | ((&fraction & 0b100000000) >> 8) as u8;
    // Take 8 bits of fraction (the missing one is in the high byte
    let lo = (&fraction & 0xFF) as u8;

    [hi, lo]
}

pub struct Ccs811Data {
    pub t_voc: u32,
    pub e_co2: u32,
    pub raw: Vec<u8>,
}

pub struct CCS811 {
    pub i2c: I2c,
}
// ------------------------------------------------------------------------

impl CCS811 {
    pub fn new(i2c: I2c) -> CCS811 {
        CCS811 { i2c }
    }

    fn reset(&mut self) -> Result<(), String> {
        self.i2c
            .block_write(CCS811_SW_RESET, &[0x11, 0xE5, 0x72, 0x8A])
            .map_err(|error| format!("Couldn't write to I2C: {}", error))?;

        sleep(CCS811_WAIT_AFTER_RESET_US);

        Ok(())
    }

    fn app_start(&mut self) -> Result<(), String> {
        self.i2c
            .write(&[CCS811_APP_START])
            .map_err(|error| format!("Could not set App start: {}", error))?;

        sleep(CCS811_WAIT_AFTER_APPSTART_US);

        Ok(())
    }

    fn erase_app(&mut self) -> Result<(), String> {
        self.i2c
            .block_write(CCS811_APP_ERASE, &[0xE7, 0xA7, 0xE6, 0x09])
            .map_err(|error| format!("Could not erase app: {}", error))?;

        sleep(CCS811_WAIT_AFTER_APPERASE_MS);

        Ok(())
    }

    fn check_hw_id(&mut self) -> Result<(), String> {
        let hw_id = self
            .i2c
            .smbus_read_byte(CCS811_HW_ID)
            .map_err(|error| format!("Couldn't read HWID: {}", error))?;

        if hw_id != 0x81 {
            return Err(format!("HWID of chip is not 0x81 but {:x?}", hw_id));
        }

        Ok(())
    }

    fn check_status(&mut self, expected: u8) -> Result<(), String> {
        let status = self
            .i2c
            .smbus_read_byte(CCS811_STATUS)
            .map_err(|error| format!("Could not read chip status: {}", error))?;

        if (status & expected) == 0 {
            return Err(format!(
                "Chip status is not {:#010b} but {:#010b}",
                expected, status
            ));
        }

        Ok(())
    }

    /// Initialize CCS811 chip with i2c bus
    /// Sequence: set i2c slave -> Wake to low -> reset chip -> check hardware id -> start chip -> check chip status -> Wake to high -> ready
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ccs811 = ccs811::new(i2c, None);
    ///
    /// match ccs811.begin() {
    ///   Ok(()) => println!("Chip is ready"),
    ///   Err(error) => panic!("Could not init the chip: {}", error)
    /// }
    /// ```
    pub fn begin(&mut self) -> Result<(), String> {
        self.i2c
            .set_slave_address(CCS811_SLAVEADDR_0)
            .map_err(|error| format!("Could not set slave addr: {}", error))?;

        self.reset()
            .and(self.check_hw_id())
            .and(self.app_start())
            .and(self.check_status(CCS811_STATUS_APP_MODE | CCS811_STATUS_APP_VERIFY))?;

        Ok(())
    }

    /// Put CCS811 chip into target mode. Be aware that the first sampled data will be available after
    /// the period of time the mode takes. For instance it will take at least 60 seconds data will be
    /// first available in the Sec60 mode. For the Sec10 mode it is at least 10 seconds etc.
    /// Also be aware that the documentation of the chip mentions to change the chip mode to a lower
    /// sampling rate like Sec1 to Sec60, the mode should be set to Idle for at least 10 minutes before
    /// the setting the new mode.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut ccs811 = ccs811::new(i2c, None);
    ///
    /// match ccs811.begin() {
    ///   Ok(()) => match ccs811.start(ccs811::MODE::Sec1) {
    ///     Ok(()) => (),
    ///     Err(error) => panic!("Could not start: {}", error)
    ///   },
    ///   Err(error) => panic!("Could not init the chip: {}", error)
    /// }
    /// ```
    pub fn start(&mut self, mode: Ccs811Mode) -> Result<(), String> {
        self.i2c
            .block_write(CCS811_MEAS_MODE, &[(mode as u8) << 4])
            .map_err(|error| format!("Could not set mode: {}", error))?;

        Ok(())
    }

    /// Version should be something like 0x1X
    pub fn hardware_version(&mut self) -> Result<u8, String> {
        self.i2c
            .smbus_read_byte(CCS811_HW_VERSION)
            .map_err(|error| format!("Could not read hardware version: {}", error))
    }

    /// Something like 0x10 0x0
    pub fn bootloader_version(&mut self) -> Result<[u8; 2], String> {
        let mut buffer = [0; 2];
        self.i2c
            .block_read(CCS811_FW_BOOT_VERSION, &mut buffer)
            .map_err(|error| format!("Could not read boot loader version: {}", error))?;

        Ok(buffer)
    }

    /// Something like 0x10 0x0 or higher. You can flash a newer firmware (2.0.0) using the flash method
    /// and a firmware binary. See examples for more details
    pub fn application_version(&mut self) -> Result<[u8; 2], String> {
        let mut buffer = [0; 2];
        self.i2c
            .block_read(CCS811_FW_APP_VERSION, &mut buffer)
            .map_err(|error| format!("Could not read application version: {}", error))?;

        Ok(buffer)
    }

    /// Get the currently used baseline
    pub fn get_baseline(&mut self) -> Result<u16, String> {
        self.i2c
            .smbus_read_word(CCS811_BASELINE)
            .map_err(|error| format!("Could not read baseline: {}", error))
    }

    /// The CCS811 chip has an automatic baseline correction based on a 24 hour interval but you still
    /// can set the baseline manually if you want.
    pub fn set_baseline(&mut self, baseline: u16) -> Result<(), String> {
        self.i2c
            .smbus_write_word(CCS811_BASELINE, baseline)
            .map_err(|error| format!("Could not set baseline: {}", error))
    }

    /// Set environmental data measured by external sensors to the chip to include those in
    /// calculations. E.g. humidity 48.5% and 23.3°C
    ///
    /// # Examples
    ///
    /// ```
    /// match ccs811.set_env_data(48.5, 23.3) {
    ///   Ok(()) => println!("Updated environmental data on chip"),
    ///   Err(error) => panic!("Failed to set environmental data on chip because {}", error)
    /// }
    /// ```
    pub fn set_env_data(&mut self, humidity: f32, temperature: f32) -> Result<(), String> {
        let data = [float_to_bytes(humidity), float_to_bytes(temperature)].concat();

        self.i2c
            .block_write(CCS811_ENV_DATA, &data)
            .map_err(|error| format!("Could npt write env data: {}", error))?;

        Ok(())
    }

    /// Read last sampled eCO2, tVOC and the corresponding status, error and raw data from the
    /// chip register
    ///
    /// # Examples
    ///
    /// ```
    /// match ccs811.read() {
    ///   Ok(data) => {
    ///     println!("t_voc: {}, e_co2: {}, raw: {:x?}", data.t_voc, data.e_co2, data.raw);
    ///   },
    ///   Err(error) => println!("Could not read data: {}", error)
    /// };
    /// ```
    pub fn read(&mut self) -> Result<Ccs811Data, String> {
        let mut buffer = [0; 8];

        self.i2c
            .block_read(CCS811_ALG_RESULT_DATA, &mut buffer)
            .map_err(|error| format!("Could not read chip data: {}", error))?;

        if buffer[5] != 0 {
            return Err(format!("Some error while reading data {:x?}", buffer[5]));
        }

        let data = Ccs811Data {
            e_co2: (buffer[0] as u16 * 256 + buffer[1] as u16) as u32,
            t_voc: (buffer[2] as u16 * 256 + buffer[3] as u16) as u32,
            raw: buffer.to_vec(),
        };

        if data.t_voc > 1187 || data.e_co2 > 8192 {
            return Err(format!(
                "The data is above max {}ppb, {}ppm",
                data.t_voc, data.e_co2
            ));
        }

        Ok(data)
    }
}
