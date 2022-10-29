use std::collections::{HashMap, HashSet};
use futures::future::join_all;
use serde::{Serialize, Deserialize};
use crate::TcpStream;

use crate::protocol::{ProtocolPackage, HasServerConnection};

type TcpStreamKeyString = String;

#[derive(Debug)]
pub struct Broker { 
    channels: HashMap<String, Vec<TcpStream>>,
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

    fn get_key(listener: &TcpStream) -> String {
        format!("{:?}", listener.peer_addr())
    }

    pub fn subscribe(&mut self, channel: String, listener: TcpStream) ->  Result<(), BrokerError>{
        let key_string = Broker::get_key(&listener);
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

    // TODO: if channel is empty, remove channel
    // TODO: send message to the channel that the user has left
    pub fn unsubscribe(&mut self, channel: &String, listener: &TcpStream) {
        let key_string = Broker::get_key(listener);
        if !self.backwards.contains_key(&key_string){
            return;
        }
        self.backwards.remove(&key_string);
        let listeners = self.channels.get_mut(channel).unwrap();
        let index = listeners.iter()
            .position(|x| Broker::get_key(x) == key_string).unwrap();
        listeners.remove(index);
    }

    pub async fn notify(&mut self, channel: &String, message: ProtocolPackage) -> Result<(), Box<bincode::ErrorKind>> {
        let serialized = bincode::serialize(&message)?;
        let listeners = self.channels.get_mut(channel).unwrap();
        // error are intentionally ignored as they need to be handled by SM which actually owns and
        // handles the TcpConnection
        let futures: Vec<_> = listeners.iter_mut()
            .map(|listener| listener.send_package_raw(&serialized))
            .collect();

        join_all(futures).await;
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
