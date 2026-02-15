use std::{path::PathBuf, sync::Arc};

use evdev::{Device, InputEvent, InputId};
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info};

pub fn start_input_scanner(mut devices_rx: mpsc::Receiver<(PathBuf, Device)>, input_sx: mpsc::Sender<InputEvent>) -> () {
    tokio::spawn(async move {
        let devices = Arc::new(Mutex::new(vec![]));
        while let Some((_, device)) = devices_rx.recv().await {
            process_device(devices.clone(), device, input_sx.clone()).await;
        }
    });
}

async fn process_device(devices: Arc<Mutex<Vec<(InputId, String)>>>, device: Device, input_sx: mpsc::Sender<InputEvent>) -> () {
    let device_not_found = {
        let devices = devices.lock().await;
        devices.iter().all(|(e, _)| {
            *e != device.input_id()
        })
    };

    if device_not_found {
        let device_name = get_device_name(&device);

        {
            let mut devices = devices.lock().await;
            info!("found new device {device_name}");

            devices.push((device.input_id(), device_name));
        }

        start_device_input_scan(devices.clone(), device, input_sx);
    }
}

fn start_device_input_scan(devices: Arc<Mutex<Vec<(InputId, String)>>>, device: Device, sx: mpsc::Sender<InputEvent>) -> () {
    tokio::spawn(async move {
        let id = device.input_id();
        let mut stream = match device.into_event_stream() {
            Ok(v) => {v},
            Err(e) => {
                error!("failed getting event stream: {e}");
                return;
            },
        };

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        loop {
            let event = match stream.next_event().await {
                Ok(v) => v,
                Err(_) => {
                    error!("error getting event from device, removing said device: {id:?}");
                    remove_device(devices, &id).await;
                    return;
                }
            };

            match sx.send(event).await {
                Ok(_) => {},
                Err(_) => {
                    error!("error sending event from device, removing said device: {id:?}");
                    remove_device(devices, &id).await;
                    return;
                },
            }
        }
    });
}

fn get_device_name(device: &Device) -> String {
    match device.name() {
        Some(v) => v.to_string(),
        None => "unknown device".to_string(),
    }
}

async fn remove_device(devices: Arc<Mutex<Vec<(InputId, String)>>>, device_id: &InputId) -> () {
    let mut devices = devices.lock().await;
    devices.retain(|(id, _)| *id != *device_id);
}
