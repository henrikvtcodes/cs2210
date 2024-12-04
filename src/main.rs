mod bmp280;
mod ccs811;
mod tmp102;

use ccs811::Ccs811Data;
use prometheus_exporter::prometheus::register_gauge;
use rppal::gpio::Gpio;
use rppal::i2c::I2c;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let i2c_voc = I2c::with_bus(1).expect("Failed to start VOC I2c!");
    let i2c_temp = I2c::with_bus(1).expect("Failed to start Temp I2c!");
    let i2c_pressure = I2c::with_bus(1).expect("Failed to start Pressure I2c!");

    let mut voc = ccs811::CCS811::new(i2c_voc);
    let mut temp = tmp102::TMP102::new(i2c_temp);
    let mut press = bmp280::BMP280::new(i2c_pressure);

    press
        .intialize()
        .expect("Failed to initialize pressure sensor");

    voc.begin().expect("Could not begin VOC sensor reading ");

    match voc.begin() {
        Ok(()) => match voc.start(ccs811::Ccs811Mode::Sec1) {
            Ok(()) => (),
            Err(error) => panic!("Could not start: {}", error),
        },
        Err(error) => panic!("Could not init the chip: {}", error),
    }

    // --- --- --- Prometheus Exporter --- --- ---
    let addr: SocketAddr = "0.0.0.0:9184".parse().unwrap();
    let exporter = prometheus_exporter::start(addr).unwrap();

    let temp_gauge = register_gauge!("temperature", "ambient temperature in celsius")
        .expect("can not create gauge temperature");
    let tvoc_gauge = register_gauge!("tvoc", "tVOC").expect("can not create gauge tvoc");
    let eco2_gauge = register_gauge!("eco2", "eCO2").expect("can not create gauge eCO2");
    let pressure_gauge = register_gauge!("pressure", "hPa").expect("can not create gauge pressure");

    loop {
        println!("Read VOC Sensor");
        match voc.read() {
            Ok(data) => {
                println!(
                    "t_voc: {}, e_co2: {}, raw: {:x?}",
                    data.t_voc, data.e_co2, data.raw
                );
            }
            Err(error) => println!("Could not read data: {}", error),
        }

        sleep(Duration::from_secs_f32(2.0));
    }

    // loop {
    //     // Will block until a new request comes in.
    //     let _guard = exporter.wait_request();
    //     println!("Updating metrics");

    //     let curr_temp = temp.read().unwrap() as f64;
    //     temp_gauge.set(curr_temp);

    //     let curr_pressure = press.read_pressure().unwrap() as f64;
    //     pressure_gauge.set(curr_pressure);

    //     let curr_voc = voc.read().unwrap();
    //     // let curr_voc = Ccs811Data {
    //     //     e_co2: 0,
    //     //     t_voc: 0,
    //     //     raw: vec![],
    //     // };
    //     tvoc_gauge.set(curr_voc.t_voc as f64);
    //     eco2_gauge.set(curr_voc.e_co2 as f64);
    // }
}
