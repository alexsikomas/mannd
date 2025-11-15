use futures::StreamExt;
use tokio::sync::mpsc::Receiver;
use tracing::info;
use zbus::proxy::SignalStream;

pub struct SignalManager<'a> {
    pub signals: Vec<SignalStream<'a>>,
}

pub enum SignalUpdate<'a> {
    Add(SignalStream<'a>),
    Remove(SignalStream<'a>),
    Clear,
}

impl<'a> SignalManager<'a> {
    pub fn new() -> Self {
        Self { signals: vec![] }
    }

    pub fn handle_update(&mut self, update: SignalUpdate<'a>) {
        match update {
            SignalUpdate::Add(stream) => {
                self.signals.push(stream);
            }
            SignalUpdate::Remove(stream) => {
                self.signals.retain(|v| v.name() != stream.name());
            }
            SignalUpdate::Clear => {
                self.signals.clear();
            }
        }
    }

    pub async fn recv(&mut self) {
        for mut signal in &mut self.signals {
            while let Some(msg) = signal.next().await {
                info!("{:?}", msg);
            }
        }
    }
}
