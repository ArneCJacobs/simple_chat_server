use std::collections::HashMap;
use futures::{AsyncWriteExt, future::join_all};
use smol::net::TcpStream;

type TcpStreamKeyString = String;
pub struct Broker { 
    channels: HashMap<String, Vec<TcpStream>>,
    backwards: HashMap<TcpStreamKeyString, String>,
}

impl Broker {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        channels.insert("welcome".to_string(), Vec::new());
        Broker {
            channels,
            backwards: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, channel: String, listener: TcpStream) {
        let key_string = format!("{:?}", listener);
        if self.backwards.contains_key(&key_string) {
            self.unsubscribe(&channel, &listener);
        } 
        if self.channels.contains_key(&channel) {
            self.channels.get_mut(&channel)
                .unwrap()
                .push(listener);
        } else {
           self.channels.insert(channel, vec![listener]);
        }
    }

    pub fn unsubscribe(&mut self, channel: &String, listener: &TcpStream) {
        let key_string = format!("{:?}", listener);
        if !self.backwards.contains_key(&key_string){
            return;
        }
        self.backwards.remove(&key_string);
        let listeners = self.channels.get_mut(channel).unwrap();
        let index = listeners.iter().position(|x| format!("{:?}", x) == key_string).unwrap();
        listeners.remove(index);
    }

    pub async fn notify(&mut self, channel: String, message: String) {
        let listeners = self.channels.get_mut(&channel).unwrap();
        let futures: Vec<_> = listeners.iter_mut()
            .map(|listener| listener.write_all((&message).as_bytes()))
            .collect();

        join_all(futures).await;
    }

    pub fn has_username(&self, username: &String) -> bool {
        self.backwards.contains_key(username)
    }
}
