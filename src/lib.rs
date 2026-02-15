use std::time::Duration;

use evdev::InputEvent;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;

pub fn start_listener() -> Result<impl Runtime> {
    #[cfg(target_os = "linux")]
    return linux::start_listener();

    #[cfg(target_os = "windows")]
    return windows::start_listener();
}

trait RuntimeInner {
    fn get_receiver(&mut self) -> impl Future<Output = mpsc::Receiver<InputEvent>>;
}

pub trait Runtime {
    fn get_receiver(&mut self) -> impl std::future::Future<Output = mpsc::Receiver<InputEvent>>;
}

impl<T> Runtime for T
where T: RuntimeInner
{
    async fn get_receiver(&mut self) -> mpsc::Receiver<InputEvent> {
        RuntimeInner::get_receiver(self).await
    }
}

pub trait Keyboard 
{
    fn tap(&self, key_code: u16) -> impl Future<Output = Result<()>>;
    fn tap_with_delay(&self, key_code: u16, delay: Duration) -> impl Future<Output = Result<()>>;
    fn press(&self, key_code: u16) -> impl Future<Output = Result<()>>;
    fn release(&self, key_code: u16) -> impl Future<Output = Result<()>>;
}

type Result<T> = core::result::Result<T, anyhow::Error>;
