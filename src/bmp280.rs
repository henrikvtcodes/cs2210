use rppal::i2c::{Error as I2CError, I2c};
use std::io::Error;

const BMP280_ADDR: u16 = 0x76; // Default I2C address
const REG_CALIBRATION_START: u8 = 0x88; // Start of Calibration Register
const REG_TEMPERATURE_START: u8 = 0xFA; // Start of Temperature Register
const REG_PRESSURE_START: u8 = 0xF7; // Start of Pressure Register

struct Calibration {
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,

    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
}

pub struct BMP280 {
    pub i2c: I2c,
    t_fine: i32,
    calib: Option<Calibration>,
}

impl BMP280 {
    pub fn new(i2c: I2c) -> Self {
        BMP280 {
            i2c,
            calib: None,
            t_fine: 0,
        }
    }

    fn set_address(&mut self) -> Result<(), I2CError> {
        self.i2c.set_slave_address(BMP280_ADDR)
    }

    fn read_calibration(&mut self) -> Result<(), Error> {
        self.set_address().expect("Failed to set I2C address");
        let mut calib_data = [0u8; 24];

        self.i2c
            .write_read(&[REG_CALIBRATION_START], &mut calib_data)
            .expect("Calibration failed during I2C");

        self.calib = Some(Calibration {
            dig_t1: u16::from_le_bytes([calib_data[0], calib_data[1]]),
            dig_t2: i16::from_le_bytes([calib_data[2], calib_data[3]]),
            dig_t3: i16::from_le_bytes([calib_data[4], calib_data[5]]),
            dig_p1: u16::from_le_bytes([calib_data[6], calib_data[7]]),
            dig_p2: i16::from_le_bytes([calib_data[8], calib_data[9]]),
            dig_p3: i16::from_le_bytes([calib_data[10], calib_data[11]]),
            dig_p4: i16::from_le_bytes([calib_data[12], calib_data[13]]),
            dig_p5: i16::from_le_bytes([calib_data[14], calib_data[15]]),
            dig_p6: i16::from_le_bytes([calib_data[16], calib_data[17]]),
            dig_p7: i16::from_le_bytes([calib_data[18], calib_data[19]]),
            dig_p8: i16::from_le_bytes([calib_data[20], calib_data[21]]),
            dig_p9: i16::from_le_bytes([calib_data[22], calib_data[23]]),
        });
        Ok(())
    }

    pub fn intialize(&mut self) -> Result<(), Error> {
        self.read_calibration()?;
        self.read_temperature()?;
        Ok(())
    }

    pub fn read_temperature(&mut self) -> Result<f32, Error> {
        let mut buffer = [0u8; 3];
        self.i2c
            .write_read(&[REG_TEMPERATURE_START], &mut buffer)
            .expect("Temp read failed on I2C");

        let calib = self
            .calib
            .as_ref()
            .expect("Temp read failed; calibration data unknown");

        let raw_temp =
            ((buffer[0] as i32) << 12) | ((buffer[1] as i32) << 4) | ((buffer[2] as i32) >> 4);

        let var1 = (((raw_temp >> 3) - ((calib.dig_t1 as i32) << 1)) * (calib.dig_t2 as i32)) >> 11;
        let var2 = (((((raw_temp >> 4) - (calib.dig_t1 as i32))
            * ((raw_temp >> 4) - (calib.dig_t1 as i32)))
            >> 12)
            * (calib.dig_t3 as i32))
            >> 14;

        self.t_fine = var1 + var2;
        let temperature = (self.t_fine * 5 + 128) >> 8;
        let temperature_c = temperature as f32 / 100.0;

        Ok(temperature_c)
    }

    pub fn read_pressure(&mut self) -> Result<f32, Error> {
        self.read_temperature()?;

        let mut buffer = [0u8; 3];
        self.i2c
            .write_read(&[REG_CALIBRATION_START], &mut buffer)
            .expect("Pressure read failed during I2C");

        let raw_press =
            ((buffer[0] as i32) << 12) | ((buffer[1] as i32) << 4) | ((buffer[2] as i32) >> 4);

        let calib = self
            .calib
            .as_ref()
            .expect("Temp read failed; calibration data unknown");

        let mut var1 = (self.t_fine as i64) - 128000;
        let mut var2 = var1 * var1 * (calib.dig_p6 as i64);
        var2 = var2 + ((var1 * (calib.dig_p5 as i64)) << 17);
        var2 = var2 + ((calib.dig_p4 as i64) << 35);
        var1 =
            ((var1 * var1 * (calib.dig_p3 as i64)) >> 8) + ((var1 * (calib.dig_p2 as i64)) << 12);
        var1 = (((1 << 47) + var1) * (calib.dig_p1 as i64)) >> 33;

        let pressure = if var1 == 0 {
            0
        } else {
            let p = 1048576 - raw_press as i64;
            let p = (((p << 31) - var2) * 3125) / var1;
            let var1 = ((calib.dig_p9 as i64) * (p >> 13) * (p >> 13)) >> 25;
            let var2 = ((calib.dig_p8 as i64) * p) >> 19;
            ((p + var1 + var2) >> 8) + ((calib.dig_p7 as i64) << 4)
        };

        let pressure_hpa = (pressure as f32) / 25600.0;
        Ok(pressure_hpa)
    }
}
