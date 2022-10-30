use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use futures::future::join_all;
use serde::{Serialize, Deserialize};

use crate::protocol::{ProtocolPackage, ProtocolPackageSender, Connection, SendReceiveError};

type TcpStreamKeyString = String;

#[derive(Debug)]
pub struct Broker { 
    channels: HashMap<String, Vec<Connection>>,
    backwards: HashMap<TcpStreamKeyString, String>,
    usernames: HashSet<String>,
}

// TODO: see specifics for send
unsafe impl Send for Broker {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BrokerError {
    UsernameAlreadyExists,
    AlreadySubscribed{ channel: String },
}


impl Broker {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        channels.insert("welcome".to_string(), Vec::new());
        Broker {
            channels,
            backwards: HashMap::new(),
            usernames: HashSet::new(),
        }
    }
    pub fn get_channels(&self) -> Vec<String> {
        self.channels.keys().cloned()
            .collect()
    }

    #[inline]
    fn get_key(listener: &std::io::Result<SocketAddr>) -> String {
        format!("{:?}", listener)
    }

    pub async fn subscribe(&mut self, channel: String, listener: Connection) ->  Result<(), BrokerError>{
        let key_string = Broker::get_key(&listener.lock().await.peer_addr());
        let res = self.backwards.get(&key_string);
        if let Some(current_channel) = res {
            return Err(BrokerError::AlreadySubscribed{ channel: current_channel.clone() });
        } 
        if self.channels.contains_key(&channel) {
            self.channels.get_mut(&channel)
                .unwrap()
                .push(listener);
        } else {
           self.channels.insert(channel.clone(), vec![listener]);
        }
        self.backwards.insert(key_string, channel);
        Ok(())
    }

    pub async fn unsubscribe(&mut self, channel: &String, listener: &Connection, username: &String) {
        let key_string = Broker::get_key(&listener.lock().await.peer_addr());
        if !self.backwards.contains_key(&key_string){
            return;
        }
        self.backwards.remove(&key_string);
        let listeners = self.channels.get_mut(channel).unwrap();
        for (index, x) in listeners.iter().enumerate() {
            let key = Broker::get_key(&x.lock().await.peer_addr());
            if key == key_string {
                listeners.remove(index);
                if listeners.is_empty() {
                    self.channels.remove(channel);
                } else {
                    let message = ProtocolPackage::ChatMessageReceive { 
                        username: "CHANNEL".to_string(), 
                        message: format!("{} HAS LEFT THE CHANNEL", username) 
                    };
                    self.notify(channel, message).await.ok();
                }
                break;
            }
        }
    }

    pub async fn notify(&mut self, channel: &String, message: ProtocolPackage) -> Result<(), SendReceiveError> {
        // let serialized = bincode::serialize(&message)?;
        let listeners = match self.channels.get_mut(channel) {
            Some(listeners) => listeners,
            None => return Err(SendReceiveError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "Channel does not exists")))
        };
        // error are intentionally ignored as they need to be handled by SM which actually owns and
        // handles the TcpConnection
        let futures: Vec<_> = listeners.iter_mut()
            .map(|listener| listener.send_package(message.clone()))
            .collect();

        for result in join_all(futures).await {
            result?;
        }
        Ok(())
    }

    pub fn register_username(&mut self, username: String) -> std::result::Result<(), BrokerError> {
        if self.usernames.contains(&username) {
            return Err(BrokerError::UsernameAlreadyExists);
        }
        self.usernames.insert(username);
        Ok(())
    }

    pub fn deregister_username(&mut self, username: &String) {
        self.usernames.remove(username);
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}
