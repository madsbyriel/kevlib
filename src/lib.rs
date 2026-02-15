use evdev::InputEvent;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;

pub type Result<T> = std::result::Result<T, anyhow::Error>;

pub fn get_runtime() -> Result<impl Runtime> {
    #[cfg(target_os = "linux")]
    return linux::get_runtime();

    #[cfg(target_os = "windows")]
    return windows::get_runtime();
}

pub trait Runtime {
    fn get_input_rx(&mut self) -> impl Future<Output = mpsc::Receiver<InputEvent>>;
}
