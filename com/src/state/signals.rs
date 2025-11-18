use futures::{
    stream::{select_all, SelectAll},
    StreamExt,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;
use zbus::{proxy::SignalStream, Message};

use crate::state::network::NetworkAction;

pub struct SignalManager<'a> {
    pub signals: SelectAll<SignalStream<'a>>,
}

pub enum SignalUpdate<'a> {
    Add(SignalStream<'a>),
    Clear,
}

impl<'a> SignalManager<'a> {
    pub fn new() -> Self {
        Self {
            signals: select_all(Vec::<SignalStream<'a>>::new()),
        }
    }

    pub fn handle_update(&mut self, update: SignalUpdate<'a>) {
        match update {
            SignalUpdate::Add(stream) => {
                self.signals.push(stream);
            }
            SignalUpdate::Clear => {
                self.signals.clear();
            }
        }
    }

    pub async fn recv(&mut self) -> Option<Message> {
        self.signals.next().await
    }

    pub async fn process_messages(&self, message: Message, tx: Sender<NetworkAction>) {
        info!("Processing");
    }
}
