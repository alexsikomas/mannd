use std::collections::HashMap;

use tokio_stream::{StreamExt, StreamMap};
use zbus::{Message, OwnedVariant, proxy::SignalStream};

use crate::state::messages::{NetworkAction, WifiAction};

#[derive(Debug)]
pub struct SignalManager<'a> {
    pub signals: StreamMap<usize, SignalStream<'a>>,
    free_keys: Vec<usize>,
    next_key: usize,
}

impl<'a> SignalManager<'a> {
    pub fn new() -> Self {
        Self {
            signals: StreamMap::<usize, SignalStream<'a>>::new(),
            free_keys: vec![],
            next_key: 1,
        }
    }

    /// Starts listening to `stream`, returning the key it is stored under
    pub fn add(&mut self, stream: SignalStream<'a>) -> usize {
        let key = self.free_keys.pop().unwrap_or_else(|| {
            self.next_key += 1;
            self.next_key - 1
        });
        self.signals.insert(key, stream);
        key
    }

    pub fn remove(&mut self, key: usize) {
        if self.signals.remove(&key).is_some() {
            self.free_keys.push(key);
        }
    }

    pub async fn recv(&mut self) -> Option<(usize, Message)> {
        self.signals.next().await
    }

    pub fn process_iwd_msg(&mut self, msg: (usize, Message)) -> Option<NetworkAction> {
        let body = msg.1.body();
        let Ok((_, changed_properties, _)) =
            body.deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>()
        else {
            return None;
        };

        // stopped scanning
        if changed_properties
            .get("Scanning")
            .is_some_and(|val| val.eq(&OwnedVariant::from(false)))
        {
            self.remove(msg.0);
            return Some(NetworkAction::Wifi(WifiAction::GetNetworks));
        }
        None
    }

    pub fn process_wpa_msg(&mut self, msg: (usize, Message)) -> Option<NetworkAction> {
        let body = msg.1.body();
        if let Some(method) = body.message().header().member() {
            match method.as_str() {
                "ScanDone" => {
                    self.remove(msg.0);
                    return Some(NetworkAction::Wifi(WifiAction::GetNetworks));
                }
                _ => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::{Connection, Proxy};

    #[tokio::test]
    async fn removed_keys_are_reused() -> zbus::Result<()> {
        let conn = Connection::system().await?;
        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await?;
        let mut manager = SignalManager::new();

        let first = manager.add(proxy.receive_signal("NameOwnerChanged").await?);
        let second = manager.add(proxy.receive_signal("NameOwnerChanged").await?);
        assert_ne!(first, second);

        manager.remove(first);
        // removing twice must not hand the same key out twice
        manager.remove(first);
        assert_eq!(manager.signals.len(), 1);

        let third = manager.add(proxy.receive_signal("NameOwnerChanged").await?);
        assert_eq!(third, first);
        let fourth = manager.add(proxy.receive_signal("NameOwnerChanged").await?);
        assert!(![first, second].contains(&fourth));
        assert_eq!(manager.signals.len(), 3);
        Ok(())
    }
}
