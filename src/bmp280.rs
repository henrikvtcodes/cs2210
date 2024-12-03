use rppal::i2c::{Error as I2CError, I2c};

const BMP280_ADDR: u16 = 0x76; // Default I2C address
const REG_CALIBRATION_START: u8 = 0x88; // Start of Calibration Register
const REG_TEMPERATURE_START: u8 = 0xFA; // Start of Temperature Register
const REG_PRESSURE_START: u8 = 0xF7; // Start of Pressure Register

pub struct BMP280 {
    pub i2c: I2c,
}

impl BMP280 {
    pub fn new(i2c: I2c) -> BMP280 {
        BMP280 { i2c }
    }

    fn set_address(&mut self) -> Result<(), I2CError> {
        self.i2c.set_slave_address(BMP280_ADDR)
    }

    pub fn read_temperature() {}
}
