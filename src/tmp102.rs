use rppal::i2c::I2c;

pub struct TMP102Data {
    pub temp: i8,
}

pub struct TMP102 {
    pub i2c: I2c,
}

impl TMP102 {
    pub fn new(i2c: I2c) {
        TMP102 { i2c }
    }

    pub fn read(&mut self) {}
}
