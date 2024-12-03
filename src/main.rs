mod bmp280;
mod ccs811;
mod tmp102;

use rppal::gpio::Gpio;
use rppal::i2c::I2c;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let i2c_voc = I2c::with_bus(1).expect("Failed to start VOC I2c!");
    // let i2c_temp = I2c::with_bus(1).expect("Failed to start Temp I2c!");
    let i2c_pressure = I2c::with_bus(1).expect("Failed to start Pressure I2c!");

    // let mut voc = ccs811::CCS811::new(i2c_voc, 14);
    // let mut temp = tmp102::TMP102::new(i2c_temp);
    let mut press = bmp280::BMP280::new(i2c_pressure);

    press
        .intialize()
        .expect("Failed to initialize pressure sensor");

    // voc.begin().expect("Could not begin VOC sensor reading ");

    // match voc.begin() {
    //     Ok(()) => match voc.start(ccs811::Ccs811Mode::Sec1) {
    //         Ok(()) => (),
    //         Err(error) => panic!("Could not start: {}", error),
    //     },
    //     Err(error) => panic!("Could not init the chip: {}", error),
    // }

    loop {
        // println!("Read VOC Sensor");
        // match voc.read() {
        //     Ok(data) => {
        //         println!(
        //             "t_voc: {}, e_co2: {}, raw: {:x?}",
        //             data.t_voc, data.e_co2, data.raw
        //         );
        //     }
        //     Err(error) => println!("Could not read data: {}", error),
        // }

        // println!("Read TMP Sensor");
        // match temp.read() {
        //     Ok(data) => {
        //         println!("Temp Celsius: {}", data)
        //     }
        //     Err(err) => {
        //         println!("Could not read temp data: {}", err)
        //     }
        // }

        print!("Read BMP280 pressure");
        match press.read_pressure() {
            Ok(data) => {
                println!("Pressure: {} kPa", data);
            }
            Err(err) => {
                println!("Could not read BMP280 pressure data: {}", err)
            }
        }
        print!("Read BMP280 temperature");
        match press.read_temperature() {
            Ok(data) => {
                println!("Temperature: {} Celsius", data);
            }
            Err(err) => {
                println!("Could not read BMP280 temperature data: {}", err)
            }
        }

        sleep(Duration::from_secs_f32(2.0));
    }

    // println!("Hello, world!");
}
