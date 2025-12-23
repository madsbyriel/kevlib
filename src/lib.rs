use std::time::Duration;

pub mod linux_keyboard;

#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    KeyboardPoisoned(linux_keyboard::KeyboardPoisonError),
}

pub trait Keyboard 
{
    fn tap(&self, key_code: u16) -> Result<()>;
    fn tap_with_delay(&self, key_code: u16, delay: Duration) -> Result<()>;
}

type Result<T> = core::result::Result<T, Error>;
