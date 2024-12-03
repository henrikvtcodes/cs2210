use rppal::i2c::I2c;
use std::error::Error;

const TMP102_ADDR: u16 = 0x48; // Default I2C address for TMP102

pub struct TMP102Data {
    pub temp: i8,
}

pub struct TMP102 {
    pub i2c: I2c,
}

impl TMP102 {
    pub fn new(i2c: I2c) -> TMP102 {
        TMP102 { i2c }
    }

    pub fn read(&mut self) -> Result<f32, Box<dyn Error>> {
        let mut buffer = [0u8; 2];

        self.i2c.set_slave_address(TMP102_ADDR);
        // Read 2 bytes from the TMP102 temperature register (register 0x00)
        self.i2c.write_read(&[0x00], &mut buffer)?;

        // Combine the two bytes into a 12-bit temperature value
        let raw_temp = ((buffer[0] as u16) << 4) | ((buffer[1] as u16) >> 4);

        // Convert raw value to temperature in Celsius
        let temp_c = if raw_temp & 0x800 == 0 {
            raw_temp as f32 * 0.0625
        } else {
            // Handle negative temperatures (two's complement)
            ((raw_temp | 0xF000) as i16) as f32 * 0.0625
        };

        Ok(temp_c)
    }
}
