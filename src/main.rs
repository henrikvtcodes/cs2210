mod ccs811;
// mod tmp102;

use rppal::gpio::Gpio;
use rppal::i2c::I2c;

fn main() {
    let i2c = I2c::with_bus(1).expect("Failed to start I2c!");

    let mut voc = ccs811::CCS811::new(i2c, 14);

    voc.begin();

    println!("{}", voc.read());

    // println!("Hello, world!");
}
