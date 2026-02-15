use std::fmt::Display;

use evdev::{Device, EventStream, InputEvent, InputId};
use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

#[derive(Debug)]
pub struct ListeningDevice {
    kill_chans: Vec<oneshot::Sender<u8>>,
    name: String,
    id: InputId,
}

impl Display for ListeningDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Drop for ListeningDevice {
    fn drop(&mut self) {
        for c in self.kill_chans.drain(..) {
            match c.send(0) {
                Ok(_) => {},
                Err(_) => {
                    error!("error sending kill signal from listening device");
                },
            }
        }

        info!("device {self} dropped");
    }
}

impl ListeningDevice {
    pub fn new(device: Device, sender: mpsc::Sender<InputEvent>) -> Self {
        let (kill_chan_sx, kill_chan_rx) = oneshot::channel::<u8>();

        let name = match device.name() {
            Some(v) => v.to_string(),
            None => "unknown device".to_string(),
        };
        let id = device.input_id();

        tokio::spawn(async move {
            info!("listening for events on {device}");
            match device.into_event_stream() {
                Ok(mut stream) => {
                    futures::select! {
                        _ = kill_chan_rx.fuse() => {},
                        _ = send_events(&mut stream, sender).fuse() => {}
                    }
                },
                Err(e) => {
                    error!("error getting event: {e}");
                },
            }
        });

        ListeningDevice { kill_chans: vec![kill_chan_sx], name, id }
    }

    pub fn input_id(&self) -> InputId {
        self.id.clone()
    }
}

async fn send_events(stream: &mut EventStream, chan: mpsc::Sender<InputEvent>) -> () {
    loop {
        match stream.next_event().await {
            Ok(input) => {
                match chan.send(input).await {
                    Ok(_) => {},
                    Err(e) => {
                        error!("error sending inputevent: {e}");
                    },
                }
            },
            Err(e) => {
                error!("error getting event: {e}");
                break;
            },
        }
    }
}
