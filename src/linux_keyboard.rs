use std::{collections::HashSet, sync::Arc, thread::sleep, time::Duration};

use evdev::{AttributeSet, Device, EventType, InputEvent, KeyCode, uinput::VirtualDevice};
use tokio::sync::{Mutex, mpsc::{self, Receiver, Sender}};
use tracing::{error, trace};
use crate::{Error, Keyboard, Result};

pub struct VirtualKeyboard {
    device: Mutex<VirtualDevice>,
}

pub struct EventBroadcaster
{
    stop_channels: Vec<Sender<u8>>,
    broadcast_channels: Arc<Mutex<Vec<Sender<InputEvent>>>>,
}

impl Drop for EventBroadcaster
{
    fn drop(&mut self) {
        for c in self.stop_channels.iter() {
            // Don't care for disconnect errors
            let _ = c.send(0);
        }
    }
}

impl EventBroadcaster
{
    pub async fn get_receiver(&self) -> Result<Receiver<InputEvent>> {
        let mut broadcasters = self.broadcast_channels.lock().await;

        let (sender, reciever) = mpsc::channel(1000);
        broadcasters.push(sender);

        Ok(reciever)
    }

    pub async fn listen_for_keys(&self, code_set: HashSet<u16>) -> Result<Option<()>> {
        let mut receiver = self.get_receiver().await?;
        let mut down_set = HashSet::new();

        while let Some(input) = receiver.recv().await {
            match input.event_type() {
                EventType::KEY => {}
                _ => continue,
            };

            let code = input.code();

            if !code_set.contains(&code) {continue};

            let value = input.value(); // documentation is lying
            let is_pressed = value == 1;
            let is_released = value == 0;

            if is_pressed {
                down_set.insert(code);
            }
            if is_released {
                down_set.remove(&code);
            }

            if code_set.is_subset(&down_set) {
                return Ok(Some(()))
            }
        }

        Ok(None)
    }

    // Listens on all keyboard-like and mouse-like devices
    // Can attach receivers to input events, otherwise they are just dropped
    pub fn start_listening(devices: Vec<Device>) -> Result<Self>
    {
        let (tx, mut rx) = mpsc::channel(1000);

        let mut stop_channels: Vec<Sender<u8>> = vec![];

        for device in devices
        {
            let tx = tx.clone();
            let (stop_sender, mut stop_receiver) = mpsc::channel(100);
            stop_channels.push(stop_sender);

            tokio::spawn(async move {
                let mut device = match device.into_event_stream() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("error getting event stream: {e}");
                        return;
                    },
                };

                loop {
                    tokio::select! {
                        v = device.next_event() => {
                            match v {
                                Ok(v) => {
                                    match tx.send(v).await {
                                        Ok(_) => {},
                                        Err(e) => {
                                            error!("error sending input event: {e}");
                                        },
                                    }
                                },
                                Err(e) => {
                                    error!("error receiving events, stopping listening thread: {e}");
                                    return;
                                },
                            }
                        },
                        _ = stop_receiver.recv() => {
                            // Just stop if we receive anything OR the channel is closed.
                            trace!("received stop signal or no senders, stopping event sender");
                            return;
                        }
                    }
                }
            });
        }

        let (s_sender, mut s_receiver) = mpsc::channel(100);
        stop_channels.push(s_sender);

        let broadcast_channels: Arc<Mutex<Vec<Sender<InputEvent>>>> = Arc::new(Mutex::new(vec![]));

        let b_channels = broadcast_channels.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    v = rx.recv() => {
                        let input = match v {
                            Some(v) => v,
                            None => {
                                error!("no input senders, stopping broadcaster");
                                return;
                            }
                        };

                        let mut channels = b_channels.lock().await;
                        let mut to_remove = vec![];

                        for (i, chan) in channels.iter().enumerate() {
                            match chan.send(input).await {
                                Ok(e) => e,
                                Err(_) => {
                                    // receiver no longer cares
                                    to_remove.push(i);
                                    continue;
                                }
                            }
                        }

                        for i in to_remove.iter().rev() {
                            channels.remove(i.clone());
                        }

                    },
                    _ = s_receiver.recv() => {
                        trace!("received stop signal or no senders, stopping input receiver");
                        return;
                    }
                };
            };
        });

        let collector = EventBroadcaster{ stop_channels: stop_channels, broadcast_channels: broadcast_channels };

        Ok(collector)
    }
}

pub fn get_devices() -> Result<Vec<Device>>
{
    let mut devices = vec![];

    trace!("getting devices");
    for (p, d) in evdev::enumerate()
    {
        let path = p.display();

        let dev = if let Some(e) = d.supported_keys() {
            if e.contains(KeyCode::KEY_A) && e.contains(KeyCode::KEY_Z)
            {
                trace!("Detected keyboard: {path}");
                Some((p, d))
            }
            else if e.contains(KeyCode::BTN_LEFT) && e.contains(KeyCode::BTN_RIGHT)
            {
                trace!("Detected mouse: {path}");
                Some((p, d))
            }
            else { None }
        } else { None };

        match dev {
            Some((_, d)) => {devices.push(d);},
            None => {},
        }
    }

    Ok(devices)
}

impl VirtualKeyboard {
    pub fn new() -> Result<Self>
    {
        let mut keys = AttributeSet::new();

        // This is a key-code integer value I found in evdev. Seems to be the highest.
        // We just want to register all keys
        for c in 0..0x2e8 {
            let key_code = KeyCode::new(c);
            keys.insert(key_code);
        }

        let device = VirtualDevice::builder()?
            .with_keys(&keys)?
            .name("Virual Keyboard")
            .build()?;

        Ok(Self{ device: Mutex::new(device) })
    }
}

impl Keyboard for VirtualKeyboard {
    async fn tap(&self, key_code: u16) -> Result<()> {
        let mut inner = self.device.lock().await;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 1)])?;
        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 0)])?;

        Ok(())
    }

    async fn tap_with_delay(&self, key_code: u16, delay: Duration) -> Result<()> {
        let mut inner = self.device.lock().await;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 1)])?;
        sleep(delay);
        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 0)])?;

        Ok(())
    }

    async fn press(&self, key_code: u16) -> Result<()>
    {
        let mut inner = self.device.lock().await;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 1)])?;

        Ok(())
    }

    async fn release(&self, key_code: u16) -> Result<()>
    {
        let mut inner = self.device.lock().await;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 0)])?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct KeyboardPoisonError;

impl From<KeyboardPoisonError> for Error {
    fn from(value: KeyboardPoisonError) -> Self {
        Error::KeyboardPoisoned(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IoError(value)
    }
}
