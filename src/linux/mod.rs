use crate::{Result, RuntimeInner, linux::listening_device::ListeningDevice};
use evdev::{Device, InputEvent};
use futures::FutureExt;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{error, info};

mod listening_device;

pub struct LinuxRuntime {
    stop_chans: Vec<oneshot::Sender<u8>>,

    multiplex_sx: Arc<Mutex<Vec<mpsc::Sender<InputEvent>>>>,
}

impl RuntimeInner for LinuxRuntime {
    async fn get_receiver(&mut self) -> mpsc::Receiver<InputEvent> {
        let (sx, rx) = mpsc::channel(100);

        let mut v = self.multiplex_sx.lock().await;
        v.push(sx);

        rx
    }
}

impl LinuxRuntime {
    fn initialize() -> Self {
        let stop_chans = vec![];
        let multiplex_sx = vec![];

        let mut runtime = LinuxRuntime {
            stop_chans,
            multiplex_sx: Arc::new(Mutex::new(multiplex_sx)),
        };

        runtime.spawn_device_listening_thread();

        runtime
    }

    fn get_stop_recv(&mut self) -> oneshot::Receiver<u8> {
        let (sx, rx) = oneshot::channel();
        self.stop_chans.push(sx);
        rx
    }

    fn send_stop_signals(&mut self) -> () {
        for sx in self.stop_chans.drain(..) {
            match sx.send(0) {
                Ok(_) => {}
                Err(_) => {
                    error!("error sending stop signal");
                }
            };
        }
    }

    fn spawn_device_listening_thread(&mut self) -> () {
        let kill_device_rx = self.get_stop_recv();
        let (sx_devices, mut rx_devices) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut stop = kill_device_rx.fuse();

            loop {
                futures::select! {
                    _ = stop => {
                        info!("stopping device scanner");
                        break;
                    },
                    _ = get_and_send_devices(&sx_devices).fuse() => {},
                }
            }
        });

        let (sx_inputs, mut rx_inputs) = mpsc::channel(100);
        tokio::spawn(async move {
            let devices = Arc::new(Mutex::new(vec![]));

            while let Some((p, d)) = rx_devices.recv().await {
                let path = match p.to_str() {
                    Some(v) => v.to_string(),
                    None => "unknown path".to_string(),
                };
                let devname = match d.name() {
                    Some(v) => v.to_string(),
                    None => "unknown device".to_string(),
                };

                if add_device(d, sx_inputs.clone(), devices.clone()).await {
                    info!("got new device {devname} at {path}");
                }
            }
        });

        let senders = self.multiplex_sx.clone();
        tokio::spawn(async move {
            while let Some(input) = rx_inputs.recv().await {
                info!("got an event");
                let senders = senders.lock().await;
                for s in senders.iter() {
                    match s.send(input).await {
                        Ok(_) => {},
                        Err(e) => {
                            error!("error sending input to listener: {e}")
                        },
                    }
                }
            }
        });
    }
}

impl Drop for LinuxRuntime {
    fn drop(&mut self) {
        self.send_stop_signals();

        info!("runtime dropped");
    }
}

fn get_devices() -> Vec<(PathBuf, Device)> {
    let mut devices = vec![];
    for (p, d) in evdev::enumerate() {
        devices.push((p, d));
    }

    devices
}

pub fn start_listener() -> Result<LinuxRuntime> {
    Ok(LinuxRuntime::initialize())
}

async fn add_device(
    device: Device,
    sender: mpsc::Sender<InputEvent>,
    devices: Arc<Mutex<Vec<ListeningDevice>>>,
) -> bool {
    let mut devices = devices.lock().await;

    let not_found = devices.iter().all(|d| d.input_id() != device.input_id());

    if not_found {
        let device = ListeningDevice::new(device, sender);
        devices.push(device);
        return true;
    }

    false
}

async fn get_and_send_devices(sender: &mpsc::Sender<(PathBuf, Device)>) -> () {
    let devices = get_devices();
    for d in devices {
        match sender.send(d).await {
            Ok(_) => {}
            Err(e) => {
                error!("error sending discovered device: {e}");
            }
        };
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
}
