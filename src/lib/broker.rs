use std::collections::{HashMap, HashSet};
use futures::future::join_all;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::SendError;

use crate::protocol::util::split_stream::SplitStream;
use crate::protocol::{ProtocolPackage, SendReceiveError};

type ConnectionIdentifier = String;

// TODO: replace Connection with split_tcp stream 
// TODO: to make sure that broker doesn't have to be put into a Arc<Mutex<_>> to be used, create a
// wrapper handler for it instead: https://tokio.rs/tokio/tutorial/shared-state#spawn-a-task-to-manage-the-state-and-use-message-passing-to-operate-on-it 
#[derive(Debug)]
pub struct Broker { 
    channels: HashMap<String, Vec<(ConnectionIdentifier, Sender<ProtocolPackage>)>>, // channel name to channel listeners
    backwards: HashMap<ConnectionIdentifier, String>, // connection identifier to channel name
    usernames: HashSet<String>,
}

// TODO: see specifics for send
unsafe impl Send for Broker {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BrokerError {
    UsernameAlreadyExists,
    AlreadySubscribed{ channel: String },
}

impl From<SendError<ProtocolPackage>> for SendReceiveError {
    fn from(_error: SendError<ProtocolPackage>) -> Self {
        SendReceiveError::IoError(
            std::io::Error::new(
                std::io::ErrorKind::Other, 
                "package failed to send"
            )
        )
    }
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

    pub async fn subscribe(&mut self, channel: String, listener: &SplitStream) ->  Result<(), BrokerError>{
        let key_string = listener.get_stream_identifier();
        let res = self.backwards.get(&key_string);
        if let Some(current_channel) = res {
            return Err(BrokerError::AlreadySubscribed{ channel: current_channel.clone() });
        } 
        let value = (key_string.clone(), listener.get_sender_clone());
        if self.channels.contains_key(&channel) {
            self.channels.get_mut(&channel)
                .unwrap()
                .push(value);
        } else {
           self.channels.insert(channel.clone(), vec![value]);
        }
        self.backwards.insert(key_string, channel);
        Ok(())
    }

    pub async fn unsubscribe(&mut self, channel: &String, listener: &SplitStream) {
        let key_string = listener.get_stream_identifier();
        if !self.backwards.contains_key(&key_string){
            return;
        }
        self.backwards.remove(&key_string);
        let listeners = self.channels.get_mut(channel).unwrap();
        for (index, (key, _)) in listeners.iter().enumerate() {
            if *key == key_string {
                listeners.remove(index);
                if listeners.is_empty() {
                    self.channels.remove(channel);
                }
                break;
            }
        }
    }

    pub async fn notify(&mut self, channel: &String, message: ProtocolPackage) -> Result<(), SendReceiveError> {
        // let serialized = bincode::serialize(&message)?;
        let listeners = match self.channels.get_mut(channel) {
            Some(listeners) => listeners,
            None => return Err(
                SendReceiveError::IoError(
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound, 
                        "Channel does not exists"
                    )
                )
            )
        };
        // error are intentionally ignored as they need to be handled by SM which actually owns and
        // handles the TcpConnection
        let futures: Vec<_> = listeners.iter_mut()
            .map(|(_, listener)| listener.send(message.clone()))
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
