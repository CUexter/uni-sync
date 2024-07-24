use hidapi::{self, HidDevice};
use serde_derive::{Deserialize, Serialize};
use std::{thread, time};

use crate::fancurve;

const VENDOR_IDS: [u16; 1] = [0x0cf2];
const PRODUCT_IDS: [u16; 7] = [0x7750, 0xa100, 0xa101, 0xa102, 0xa103, 0xa104, 0xa105];

#[derive(Serialize, Deserialize, Clone)]
pub struct Configs {
    pub configs: Vec<Config>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub device_id: String,
    pub sync_rgb: bool,
    pub channels: Vec<Channel>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Channel {
    pub mode: String,
    pub speed: usize,
    pub fan_curve: Option<String>,
}

pub fn run(existing_configs: Configs) -> Configs {
    let mut updated_configs = existing_configs.clone();

    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(_) => {
            eprintln!("Could not find any controllers");
            return updated_configs;
        }
    };

    for hiddevice in api.device_list() {
        if VENDOR_IDS.contains(&hiddevice.vendor_id()) && PRODUCT_IDS.contains(&hiddevice.product_id()) {
            let serial_number: &str = hiddevice.serial_number().unwrap();
            let device_id: String = format!(
                "VID:{}/PID:{}/SN:{}",
                hiddevice.vendor_id(),
                hiddevice.product_id(),
                serial_number
            );

            let hid: HidDevice = match api.open(hiddevice.vendor_id(), hiddevice.product_id()) {
                Ok(hid) => hid,
                Err(_) => {
                    eprintln!("Please run uni-sync with elevated permissions.");
                    continue;
                }
            };

            if let Some(config) = updated_configs.configs.iter_mut().find(|c| c.device_id == device_id) {
                update_device(&hid, hiddevice.product_id(), config);
            } else {
                let mut new_config = create_default_config(device_id);
                update_device(&hid, hiddevice.product_id(), &mut new_config);
                updated_configs.configs.push(new_config);
            }
        }
    }

    updated_configs
}

fn update_device(hid: &HidDevice, product_id: u16, config: &mut Config) {
    // Set RGB sync
    let sync_byte: u8 = if config.sync_rgb { 1 } else { 0 };
    let _ = match product_id {
        0xa100 | 0x7750 => hid.write(&[224, 16, 48, sync_byte, 0, 0, 0]),
        0xa101 => hid.write(&[224, 16, 65, sync_byte, 0, 0, 0]),
        0xa102 | 0xa103 | 0xa104 | 0xa105 => hid.write(&[224, 16, 97, sync_byte, 0, 0, 0]),
        _ => hid.write(&[224, 16, 48, sync_byte, 0, 0, 0]),
    };
    thread::sleep(time::Duration::from_millis(200));

    // Update each channel
    for (x, channel) in config.channels.iter_mut().enumerate() {
        let mut channel_byte = 0x10 << x;
        if channel.mode == "PWM" {
            channel_byte |= 0x1 << x;
        }

        let _ = match product_id {
            0xa100 | 0x7750 => hid.write(&[224, 16, 49, channel_byte]),
            0xa101 => hid.write(&[224, 16, 66, channel_byte]),
            0xa102 | 0xa103 | 0xa104 | 0xa105 => hid.write(&[224, 16, 98, channel_byte]),
            _ => hid.write(&[224, 16, 49, channel_byte]),
        };
        thread::sleep(time::Duration::from_millis(200));

        match channel.mode.as_str() {
            "fan-curve" => {
                if let Some(fan_curve_file) = &channel.fan_curve {
                    if let Ok(fan_curve) = fancurve::read_fan_curve(fan_curve_file) {
                        if let Ok(temperature) = fancurve::get_current_temperature(&fan_curve.sensor) {
                            let speed = fancurve::calculate_fan_speed(&fan_curve, temperature);
                            set_fan_speed(hid, product_id, x, speed);
                            channel.speed = speed;
                        }
                    }
                }
            },
            "Manual" => set_fan_speed(hid, product_id, x, channel.speed),
            "PWM" => (), // PWM mode is set by the channel_byte above
            _ => eprintln!("Unknown mode for channel {}: {}", x, channel.mode),
        }
    }
}

fn create_default_config(device_id: String) -> Config {
    Config {
        device_id,
        sync_rgb: false,
        channels: vec![
            Channel { mode: "Manual".to_string(), speed: 50, fan_curve: None },
            Channel { mode: "Manual".to_string(), speed: 50, fan_curve: None },
            Channel { mode: "Manual".to_string(), speed: 50, fan_curve: None },
            Channel { mode: "Manual".to_string(), speed: 50, fan_curve: None },
        ],
    }
}

fn set_fan_speed(hid: &HidDevice, product_id: u16, channel: usize, speed: usize) {
    let speed_byte = match product_id {
        0xa100 | 0x7750 | 0xa101 => ((800.0 + (11.0 * speed as f64)) as usize / 19) as u8,
        0xa102 => ((200.0 + (19.0 * speed as f64)) as usize / 21) as u8,
        0xa103 | 0xa104 | 0xa105 => ((250.0 + (17.5 * speed as f64)) as usize / 20) as u8,
        _ => ((800.0 + (11.0 * speed as f64)) as usize / 19) as u8,
    };

    let _ = hid.write(&[224, (channel + 32) as u8, 0, speed_byte]);
    thread::sleep(time::Duration::from_millis(100));
}
