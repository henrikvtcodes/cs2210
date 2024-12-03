mod ccs811;
// mod tmp102;

use rppal::gpio::Gpio;
use rppal::i2c::I2c;

fn main() {
    let i2c = I2c::with_bus(1).expect("Failed to start I2c!");

    let mut voc = ccs811::CCS811::new(i2c, 14);

    voc.begin();

    loop {
        match voc.read() {
            Ok(data) => {
                println!(
                    "t_voc: {}, e_co2: {}, raw: {:x?}",
                    data.t_voc, data.e_co2, data.raw
                );
            }
            Err(error) => println!("Could not read data: {}", error),
        }
    }

    // println!("Hello, world!");
}
