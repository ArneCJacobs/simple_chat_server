use crate::protocol::{ProtocolPackageSender, ProtocolPackageReader};
use std::{sync::Arc, fmt::Debug};

use rust_state_machine::{AsyncProgress, ToStatesAndOutput, AsyncToStatesAndOutput, state_machine, with_context, async_with_context};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use crate::{broker::{Broker, BrokerError}, impl_send_receive};
use super::{ProtocolPackage, SendReceiveError, util::split_stream::SplitStream};

// TODO: remove usage of Arc<Mutex<TcpStream>> in all server states and replace with split_stream
#[derive(Debug)]
pub struct ClientConnection{ 
    pub socket: SplitStream,
}
#[derive(Debug)]
pub struct ClientConnectionAuthenticated{ 
    broker: Arc<Mutex<Broker>>,
    socket: SplitStream,
    username: String 
}
#[derive(Debug)]
pub struct ClientChannelConnection{ 
    broker: Arc<Mutex<Broker>>,
    socket: SplitStream,
    username: String, 
    channel: String 
}
pub struct Disconnected;
pub struct SharedContext{ 
    pub broker: Arc<Mutex<Broker>> 
} 

#[derive(Debug)]
pub enum Reaction {
    Success,
    Disconnected,
    LostConnecion,
    MalformedPackage,
    UsernameAlreadyTaken,
    AlreadyConnectedToChannel{ channel: String },
    IoError(std::io::Error),
    BinError(Box<bincode::ErrorKind>),
}
type Input = ();

state_machine!{
    pub
    async
    Name(ServerSideConnectionSM)
    Start(ClientConnection)
    SharedContext(SharedContext)
    Input(Input)
    Output(Reaction)
    Edges {
        ClientConnection => [Disconnected, ClientConnection, ClientConnectionAuthenticated],
        ClientConnectionAuthenticated => [Disconnected, ClientConnectionAuthenticated, ClientChannelConnection],
        ClientChannelConnection => [Disconnected, ClientChannelConnection, ClientConnectionAuthenticated],
    }
}

impl From<SendReceiveError> for Reaction {
    fn from(error: SendReceiveError) -> Self {
        match error {
            SendReceiveError::IoError(error) => Reaction::IoError(error),
            SendReceiveError::BinError(error) => Reaction::BinError(error),
        }
    }
}

impl_send_receive!(Reaction, Disconnected, Disconnected, ClientConnection, ClientChannelConnection);


impl ToStatesAndOutput<ClientConnection, ClientConnectionEdges, Reaction> for BrokerError {
    fn context(self, state: ClientConnection) -> (ClientConnectionEdges, Reaction) {
        match self {
            BrokerError::UsernameAlreadyExists => (state.into(), Reaction::UsernameAlreadyTaken),
            _ => todo!() // this should never be possible;
        }
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientConnectionEdges, ServerSideConnectionSM> for ClientConnection {
   async fn transition(mut self, shared: &mut SharedContext, _: ()) -> Option<(ClientConnectionEdges, Reaction)> {
        let input = with_context!(self.socket.receive_package().await, self);
        let username = match input {
            ProtocolPackage::ServerAuthenticationRequest{ username } => username,
            ProtocolPackage::DisconnectNotification => return Some((Disconnected.into(), Reaction::Disconnected)),
            _ => return Some((self.into(), Reaction::MalformedPackage)),
        };

        let mut guard = shared.broker.lock().await;               
        let res = guard.register_username(username.clone());
        std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 
        if let Err(error) = res {
            let message = ProtocolPackage::Deny{ error: error.clone() };
            // with_context!(self.socket.send(message), self);
            with_context!(self.socket.send_package(message).await, self);
            with_context!(Err(error), self)
        } else {
            let reply = ProtocolPackage::Accept;
            with_context!(self.socket.send_package(reply).await, self);
            let new_state = ClientConnectionAuthenticated {
                broker: shared.broker.clone(),
                socket: self.socket,
                username,
            };
            return Some((new_state.into(), Reaction::Success));
        }
   } 
}

impl ToStatesAndOutput<ClientConnectionAuthenticated, ClientConnectionAuthenticatedEdges, Reaction> for BrokerError {
    fn context(self, state: ClientConnectionAuthenticated) -> (ClientConnectionAuthenticatedEdges, Reaction) {
        match self {
            BrokerError::AlreadySubscribed{ channel } => (state.into(), Reaction::AlreadyConnectedToChannel{ channel }),
            BrokerError::UsernameAlreadyExists => todo!() // this should never be possible,
        }
    }
} 

#[::rust_state_machine::async_trait::async_trait]
impl AsyncToStatesAndOutput<ClientConnectionAuthenticated, ClientConnectionAuthenticatedEdges, Reaction> for std::io::Error {
    async fn context(self, state: ClientConnectionAuthenticated) -> (ClientConnectionAuthenticatedEdges, Reaction) {
        state.shutdown().await;
        (Disconnected.into(), Reaction::IoError(self))
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncToStatesAndOutput<ClientConnectionAuthenticated, ClientConnectionAuthenticatedEdges, Reaction> for SendReceiveError {
    async fn context(self, state: ClientConnectionAuthenticated) -> (ClientConnectionAuthenticatedEdges, Reaction) {
        match self {
            SendReceiveError::IoError(error) => AsyncToStatesAndOutput::context(error, state).await,
            SendReceiveError::BinError(error) => (state.into(), error.into())
        }
    }
}

impl From<Box<bincode::ErrorKind>> for Reaction {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        Reaction::BinError(error)
    }
}

impl ToStatesAndOutput<ClientConnectionAuthenticated, ClientConnectionAuthenticatedEdges, Reaction> for SendReceiveError {
    fn context(self, state: ClientConnectionAuthenticated) -> (ClientConnectionAuthenticatedEdges, Reaction) {
        match self {
            SendReceiveError::BinError(_) => (state.into(), self.into()),
            SendReceiveError::IoError(ref error) => match error.kind() {
                std::io::ErrorKind::NotFound => (state.into(), self.into()),
                _ => panic!("Should never be reached")
            }
        }
        
    }
}


impl ClientConnectionAuthenticated {
    async fn list_channels(mut self, shared: &mut SharedContext) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)>
    {
        let guard = shared.broker.lock().await;
        let channels = guard.get_channels();
        std::mem::drop(guard);
        let message = ProtocolPackage::InfoListChannelsReply{ channels };

        async_with_context!(self.socket.send_package(message).await, self);

        Some((self.into(), Reaction::Success))
    }

    async fn join_channel(mut self, shared: &mut SharedContext, channel: String) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)> 
    {
        let mut guard = shared.broker.lock().await;
        let res = guard.subscribe(channel.clone(), &self.socket).await;
        drop(guard);
        if let Err(error) = res {
            let message = ProtocolPackage::Deny{ error: error.clone() };
            async_with_context!(self.socket.send_package(message).await, self);
            with_context!(Err(error), self);
        }
        let message = ProtocolPackage::Accept;
        async_with_context!(self.socket.send_package(message).await, self);

        let message = ProtocolPackage::ChatMessageReceive { 
            username: "CHANNEL".to_string(), 
            message: format!("{} JOINED CHANNEL", self.username) 
        };
        let mut guard = shared.broker.lock().await;
        with_context!(guard.notify(&channel, message).await, self);
        std::mem::drop(guard);

        let new_state = ClientChannelConnection {
            broker: self.broker,
            socket: self.socket,
            username: self.username,
            channel,
        };
        Some((new_state.into(), Reaction::Success))
    }

    pub async fn shutdown(self) {
        // Don't care if successful or not, either way the channel
        // will be disconnected
        self.socket.unsplit().await
            .shutdown().await
            .ok();
        let mut guard = self.broker.lock().await;
        guard.deregister_username(&self.username);
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientConnectionAuthenticatedEdges, ServerSideConnectionSM> for ClientConnectionAuthenticated {
    async fn transition(mut self, shared: &mut SharedContext, _: ()) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)> {
        let input = async_with_context!(self.socket.receive_package().await, self);
        match input {
            ProtocolPackage::InfoListChannelsRequest => self.list_channels(shared).await,
            ProtocolPackage::ChannelConnectionRequest{ channel } => self.join_channel(shared, channel).await,
            ProtocolPackage::DisconnectNotification => {
                self.shutdown().await;
                return Some((Disconnected.into(), Reaction::Disconnected))
            },
            _ => return Some((self.into(), Reaction::MalformedPackage)),
        }
    } 
}

impl ToStatesAndOutput<ClientChannelConnection, ClientChannelConnectionEdges, Reaction> for Box<bincode::ErrorKind> {
    fn context(self, state: ClientChannelConnection) -> (ClientChannelConnectionEdges, Reaction) {
        (state.into(), self.into())
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncToStatesAndOutput<ClientChannelConnection, ClientChannelConnectionEdges, Reaction> for std::io::Error {
    async fn context(self, state: ClientChannelConnection) -> (ClientChannelConnectionEdges, Reaction) {
        state.shutdown(false).await;
        (Disconnected.into(), Reaction::LostConnecion)
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncToStatesAndOutput<ClientChannelConnection, ClientChannelConnectionEdges, Reaction> for SendReceiveError {
    async fn context(self, state: ClientChannelConnection) -> (ClientChannelConnectionEdges, Reaction) {
        match self {
            SendReceiveError::BinError(error) => ToStatesAndOutput::context(error, state),
            SendReceiveError::IoError(error) => AsyncToStatesAndOutput::context(error, state).await,
        }
    }
}

impl ClientChannelConnection {
    async fn send_message(mut self, shared: &mut SharedContext, message: String) -> Option<(ClientChannelConnectionEdges, Reaction)> {
        let message = ProtocolPackage::ChatMessageReceive { username: self.username.clone(), message };
        let broker = shared.broker.clone();
        let channel = self.channel.clone();

        tokio::spawn(async move {
            let mut guard = broker.lock().await;
            guard.notify(&channel, message).await.unwrap();
            std::mem::drop(guard);
            std::mem::drop(broker);
        });

        let message = ProtocolPackage::Accept;
        async_with_context!(self.socket.send_package(message).await, self);
        Some((self.into(), Reaction::Success))
    } 

    pub async fn shutdown(self, intentional: bool) {
        let mut guard = self.broker.lock().await;
        guard.unsubscribe(&self.channel, &self.socket, &self.username).await;
        guard.deregister_username(&self.username);

        // Don't care if successful or not, either way the channel
        // will be disconnected
        self.socket.unsplit().await
            .shutdown().await
            .ok();
 
        if guard.get_channels().contains(&self.channel) {
            let message_text = if intentional {
                format!("{} DISCONNECTED", self.username)
            } else {
                format!("{} LOST CONNECTION", self.username)
            };
            let message = ProtocolPackage::ChatMessageReceive { 
                username: "CHANNEL".to_string(), 
                message: message_text 
            };

            guard.notify(&self.channel, message).await
                .expect("Could not serialize message");
        }

        std::mem::drop(guard);
    }
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientChannelConnectionEdges, ServerSideConnectionSM> for ClientChannelConnection {
    async fn transition(mut self, shared: &mut SharedContext, _: ()) -> Option<(ClientChannelConnectionEdges, Reaction)> {
        let input = async_with_context!(self.socket.receive_package().await, self);
        match input {
            ProtocolPackage::ChatMessageSend{ message } => self.send_message(shared, message).await, 
            ProtocolPackage::ChannelDisconnectNotification => {
                let message = ProtocolPackage::Accept;
                with_context!(self.socket.send_package(message).await, self);
                let mut guard = shared.broker.lock().await;
                guard.unsubscribe(&self.channel, &self.socket, &self.username).await;
                std::mem::drop(guard);
                let new_state = ClientConnectionAuthenticated {
                    username: self.username,
                    socket: self.socket,
                    broker: self.broker,
                };
                return Some((new_state.into(), Reaction::Success))
                
            },
            ProtocolPackage::DisconnectNotification => {
                self.shutdown(true).await;
                return Some((Disconnected.into(), Reaction::Disconnected))
            },
            _ => return Some((self.into(), Reaction::MalformedPackage)),
        }
    } 
}
