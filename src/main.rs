mod ccs811;
mod tmp102;

use rppal::gpio::Gpio;
use rppal::i2c::I2c;
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let i2c_voc = I2c::with_bus(1).expect("Failed to start VOC I2c!");
    let i2c_temp = I2c::with_bus(1).expect("Failed to start Temp I2c!");

    let mut voc = ccs811::CCS811::new(i2c_voc, 14);
    let mut temp = tmp102::TMP102::new(i2c_temp);

    voc.begin().expect("Could not begin VOC sensor reading ");

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

        match temp.read() {
            Ok(data) => {
                println!("Temp Celsius: {}", data)
            }
            Err(err) => {
                println!("Could not read temp data: {}", err)
            }
        }

        sleep(Duration::from_secs_f32(2.0));
    }

    // println!("Hello, world!");
}
