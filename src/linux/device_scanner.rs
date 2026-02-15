use std::{path::PathBuf, time::Duration};

use evdev::{Device, KeyCode};
use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

pub fn start_device_scanner(
    sx: mpsc::Sender<(PathBuf, Device)>,
    kill: oneshot::Receiver<u8>,
) -> () {
    tokio::spawn(async move {
        futures::select! {
            _ = scan(sx).fuse() => {},
            _ = kill.fuse() => {}
        }
    });
}

async fn scan(sx: mpsc::Sender<(PathBuf, Device)>) -> () {
    loop {
        for (path, device) in evdev::enumerate() {
            match sx.send((path, device)).await {
                Ok(_) => {}
                Err(e) => {
                    error!("error sending device, shutting down device listener: {e}");
                    return;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
