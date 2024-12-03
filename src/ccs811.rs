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

pub struct CCS811ErrorRegister {
    pub error_present: bool,
    pub write_reg_invalid: bool,
    pub read_reg_invalid: bool,
    pub meas_mode_invalid: bool,
    pub max_resistance: bool,
    pub heater_fault: bool,
    pub heater_supply: bool,
}

// impl fmt::Display for CCS811ErrorRegister {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         let mut fmt_str = "";
//         if (self.error_present) {
//             fmt_str = format!( "error_present: {}\nwrite_reg_invalid: {}\nread_reg_invalid: {}\nmeas_mode_invalid: {}\nmax_resistance: {}\nmax_resistance: {}\nheater_fault: {}\nheater_supply: {}",
//                 self.error_present,self.write_reg_invalid,self.read_reg_invalid,self.meas_mode_invalid, self.max_resistance, self.heater_fault, self.heater_supply);
//         } else {
//             fmt_str = "No error";
//         }
//         write(f, "{}", fmt_str)
//     }
// }

pub struct CCS811 {
    pub i2c: I2c,
}

impl CCS811 {
    fn reset(&mut self) -> Result<(), String> {
        self.i2c
            .block_write(CCS811_SW_RESET, &[0x11, 0xE5, 0x72, 0x8A])
            .map_err(|error| format!("Couldn't write to I2C: {}", error))?;

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

    pub fn new(i2c: I2c) -> CCS811 {
        CCS811 { i2c }
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

    pub fn check_error(&mut self) -> Result<CCS811ErrorRegister, String> {
        let mut buffer = [0u8; 1];

        let _ = self
            .i2c
            .block_read(CCS811_STATUS, &mut buffer)
            .map_err(|error| format!("Could not read status: {}", error))?;

        let mut status = buffer[0];

        let error = (status & 0b0000_0001) != 0; // Bit 0: ERROR (1 = error occurred)

        let _ = self
            .i2c
            .block_read(CCS811_ERR, &mut buffer)
            .map_err(|error| format!("Could not read status: {}", error))?;

        // Read status byte from error register
        status = buffer[0];

        if error {
            Ok(CCS811ErrorRegister {
                error_present: true,
                write_reg_invalid: (status & 0b0000_0001) != 0,
                read_reg_invalid: false,
                meas_mode_invalid: false,
                max_resistance: (status & 0b0000_1000) != 0,
                heater_fault: (status & 0b0001_0000) != 0,
                heater_supply: false,
            })
        } else {
            Ok(CCS811ErrorRegister {
                error_present: false,
                write_reg_invalid: false,
                read_reg_invalid: false,
                meas_mode_invalid: false,
                max_resistance: false,
                heater_fault: false,
                heater_supply: false,
            })
        }
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
        let mut buffer = [0u8; 8];
        self.i2c
            .block_read(CCS811_ALG_RESULT_DATA, &mut buffer)
            .expect("VOC read failed during i2c");

        let co2 = u32::from_be_bytes([0, buffer[0], buffer[1], 0]);
        let tvoc = u32::from_be_bytes([0, buffer[2], buffer[3], 0]);

        // if buffer[5] != 0 {
        //     return Err(format!("Some error while reading data {:x?}", buffer[5]));
        // }

        let data = Ccs811Data {
            e_co2: co2,
            t_voc: tvoc,
            raw: buffer.to_vec(),
        };

        // if data.t_voc > 1187 || data.e_co2 > 8192 {
        //     return Err(format!(
        //         "The data is above max {}ppb, {}ppm",
        //         data.t_voc, data.e_co2
        //     ));
        // }

        Ok(data)
    }
}
