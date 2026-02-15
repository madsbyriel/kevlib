use std::{path::PathBuf, time::Duration};

use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{error};

pub fn start_device_scanner(
    sx: mpsc::Sender<PathBuf>,
    kill: oneshot::Receiver<()>,
) -> () {
    tokio::spawn(async move {
        futures::select! {
            _ = scan(sx).fuse() => {},
            _ = kill.fuse() => {}
        }
    });
}

async fn scan(sx: mpsc::Sender<PathBuf>) -> () {
    loop {
        for (path, _) in evdev::enumerate() {
            match sx.send(path).await {
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
