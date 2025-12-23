use std::{sync::Mutex, thread::sleep, time::Duration};

use evdev::{AttributeSet, EventType, InputEvent, KeyCode, uinput::VirtualDevice};
use crate::{Error, Keyboard, Result};

pub struct VirtualKeyboard {
    device: Mutex<VirtualDevice>,
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
    fn tap(&self, key_code: u16) -> Result<()> {
        let mut inner = self.device.lock().map_err(|_| KeyboardPoisonError{})?;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 1)])?;
        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 0)])?;

        Ok(())
    }

    fn tap_with_delay(&self, key_code: u16, delay: Duration) -> Result<()> {
        let mut inner = self.device.lock().map_err(|_| KeyboardPoisonError{})?;

        inner.emit(&[InputEvent::new(EventType::KEY.0, key_code, 1)])?;
        sleep(delay);
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
