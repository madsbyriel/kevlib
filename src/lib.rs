use evdev::InputEvent;
use tokio::sync::mpsc;
use tracing::info;

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


pub async fn test() {
    for (_, d) in evdev::enumerate() {
        tokio::spawn(async move {
            let mut stream = d.into_event_stream().unwrap();
            while let Ok(event) = stream.next_event().await {
                info!("EVENT: {event:?}");
            }
        });
    }
}
