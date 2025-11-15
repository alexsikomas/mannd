use futures::StreamExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;
use zbus::{proxy::SignalStream, Message};

use crate::state::network::NetworkAction;

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

    pub async fn recv(&mut self, tx: Sender<NetworkAction>) {
        let mut messages = vec![];
        for signal in &mut self.signals {
            while let Some(msg) = signal.next().await {
                info!("The signal recieved was: {:?}", msg);
                messages.push(msg);
            }
        }
        if !messages.is_empty() {
            self.process_messages(messages, tx);
        }
    }

    pub async fn process_messages(&self, messages: Vec<Message>, tx: Sender<NetworkAction>) {
        for msg in messages {
            info!("{msg}");
        }
    }
}
