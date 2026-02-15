use std::{path::PathBuf, sync::Arc};

use evdev::{Device, InputEvent};
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info};

pub fn start_input_scanner(mut devices_rx: mpsc::Receiver<PathBuf>, input_sx: mpsc::Sender<InputEvent>) -> () {
    tokio::spawn(async move {
        let devices = Arc::new(Mutex::new(vec![]));

        while let Some(path) = devices_rx.recv().await {
            process_device(devices.clone(), path, input_sx.clone()).await;
        }

        info!("input scanner shut down");
    });
}

async fn process_device(devices: Arc<Mutex<Vec<PathBuf>>>, path: PathBuf, input_sx: mpsc::Sender<InputEvent>) -> () {
    let device_not_found = {
        let devices = devices.lock().await;
        devices.iter().all(|e| {
            *e != path
        })
    };

    if device_not_found {
        let device = match Device::open(path.clone()) {
            Ok(v) => v,
            Err(e) => {
                let path = get_path(&path);
                error!("error opening device at {path}: {e}");
                return;
            },
        };

        let device_name = get_device_name(&device);
        {
            let mut devices = devices.lock().await;
            info!("found new device {device_name}");

            devices.push(path.clone());
        }

        start_device_input_scan(devices.clone(), device, path, input_sx);
    }
}

fn start_device_input_scan(devices: Arc<Mutex<Vec<PathBuf>>>, device: Device, device_path: PathBuf, sx: mpsc::Sender<InputEvent>) -> () {
    tokio::spawn(async move {
        let id = device.input_id();
        let device_name = get_device_name(&device);

        let mut stream = match device.into_event_stream() {
            Ok(v) => {v},
            Err(e) => {
                error!("failed getting event stream for {device_name}: {e}");
                return;
            },
        };

        loop {
            let event = match stream.next_event().await {
                Ok(v) => v,
                Err(_) => {
                    error!("error getting event from device {device_name}, removing said device: {id:?}");
                    remove_device(devices, &device_path).await;
                    return;
                }
            };

            match sx.send(event).await {
                Ok(_) => {},
                Err(_) => {
                    error!("error sending event from device {device_name}, removing said device: {id:?}");
                    remove_device(devices, &device_path).await;
                    return;
                },
            }
        }
    });
}

fn get_device_name(device: &Device) -> String {
    device.name().unwrap_or("unknown device").to_string()
}

fn get_path(path: &PathBuf) -> String {
    path.to_str().unwrap_or("unknown path").to_string()
}

async fn remove_device(devices: Arc<Mutex<Vec<PathBuf>>>, device_path: &PathBuf) -> () {
    let mut devices = devices.lock().await;
    devices.retain(|path| *path != *device_path);
}
