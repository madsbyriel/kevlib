use std::sync::Arc;

use evdev::InputEvent;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::error;

use crate::{Result, Runtime};

mod device_scanner;
mod input_scanner;

pub fn get_runtime() -> Result<impl Runtime> {
    Ok(LinuxRuntime::initialize())
}

struct LinuxRuntime {
    input_sxs: Arc<Mutex<Vec<mpsc::Sender<InputEvent>>>>,
    kill_sxs: Vec<oneshot::Sender<u8>>,
}

impl Drop for LinuxRuntime {
    fn drop(&mut self) {
        for c in self.kill_sxs.drain(..) {
            match c.send(0) {
                Ok(_) => {},
                Err(_) => {},
            };
        }
    }
}

impl LinuxRuntime {
    fn initialize() -> Self {
        let input_sxs = Arc::new(Mutex::new(vec![]));
        let (device_kill_sx, device_kill_rx) = oneshot::channel();
        let kill_sxs = vec![device_kill_sx];

        let (sx_devices, rx_devices) = mpsc::channel(100);
        device_scanner::start_device_scanner(sx_devices, device_kill_rx);

        let (sx_input, rx_input) = mpsc::channel(100);
        input_scanner::start_input_scanner(rx_devices, sx_input);

        start_input_sender(rx_input, input_sxs.clone());

        LinuxRuntime { input_sxs, kill_sxs }
    }

    async fn get_input_rx(&mut self) -> mpsc::Receiver<InputEvent> {
        let mut sxs = self.input_sxs.lock().await;
        let (sx, rx) = mpsc::channel(100);
        sxs.push(sx);
        rx
    }
}

fn start_input_sender(mut rx: mpsc::Receiver<InputEvent>, sxs: Arc<Mutex<Vec<mpsc::Sender<InputEvent>>>>) -> () {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let senders = sxs.lock().await;

            for s in senders.iter() {
                match s.send(event).await {
                    Ok(_) => {},
                    Err(e) => {
                        error!("error sending event, shutting down input sender: {e}");
                        return;
                    }
                }
            }
        }
    });
}

impl Runtime for LinuxRuntime {
    async fn get_input_rx(&mut self) -> mpsc::Receiver<InputEvent> {
        LinuxRuntime::get_input_rx(self).await
    }
}
