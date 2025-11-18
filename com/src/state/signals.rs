use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::{StreamExt, StreamMap};
use tracing::info;
use zbus::{names::MemberName, proxy::SignalStream, Message};

use crate::state::network::NetworkAction;

pub struct SignalManager<'a> {
    pub signals: StreamMap<usize, SignalStream<'a>>,
    free_keys: Vec<usize>,
    next_key: usize,
}

pub enum SignalUpdate<'a> {
    Add(SignalStream<'a>),
    Remove(usize),
    Clear,
}

impl<'a> SignalManager<'a> {
    pub fn new() -> Self {
        Self {
            signals: StreamMap::<usize, SignalStream<'a>>::new(),
            free_keys: vec![],
            next_key: 1,
        }
    }

    pub fn handle_update(&mut self, update: SignalUpdate<'a>) {
        match update {
            SignalUpdate::Add(stream) => {
                let use_key = if let Some(key) = self.free_keys.pop() {
                    key
                } else {
                    let key = self.next_key;
                    self.next_key += 1;
                    key
                };
                self.signals.insert(use_key, stream);
            }
            SignalUpdate::Remove(i) => {
                if self.signals.remove(&i).is_some() {
                    self.free_keys.push(i);
                }
            }
            SignalUpdate::Clear => {
                self.signals.clear();
                self.free_keys.clear();
                self.next_key = 1;
            }
        }
    }

    pub async fn recv(&mut self) -> Option<(usize, Message)> {
        self.signals.next().await
    }

    pub async fn process_iwd_msg(&mut self, msg: (usize, Message), tx: Sender<NetworkAction>) {}
    pub async fn process_wpa_msg(&mut self, msg: (usize, Message), tx: Sender<NetworkAction>) {
        let body = msg.1.body();
        if let Some(method) = body.message().header().member() {
            info!("PROCESSING: {:?}", method);
            match method.as_str() {
                "ScanDone" => {
                    tx.send(NetworkAction::GetKnownNetworks);
                    self.handle_update(SignalUpdate::Remove(msg.0));
                }
                _ => {}
            };
        }
    }
}
