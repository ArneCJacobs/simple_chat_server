use std::{sync::Arc, fmt::Debug};

use rust_state_machine::{AsyncProgress, ToStatesAndOutput, AsyncToStatesAndOutput, state_machine, with_context, async_with_context};
use smol::{net::TcpStream, lock::Mutex};
use crate::{broker::{Broker, BrokerError}, impl_send_receive};
use super::{HasServerConnection, ProtocolPackage, SendReceiveError};

// ### STATES ###
#[derive(Debug, Clone)]
pub struct ClientConnection{ 
    pub socket: TcpStream 
}
#[derive(Debug, Clone)]
pub struct ClientConnectionAuthenticated{ 
    broker: Arc<Mutex<Broker>>,
    socket: TcpStream, 
    username: String 
}
#[derive(Debug, Clone)]
pub struct ClientChannelConnection{ 
    broker: Arc<Mutex<Broker>>,
    socket: TcpStream, 
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

        let mut guard = shared.broker.lock_arc().await;               
        let res = guard.register_username(username.clone());
        std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 
        if let Err(error) = res {
            let message = ProtocolPackage::Deny{ error: error.clone() };
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



impl ClientConnectionAuthenticated {
    async fn list_channels(mut self, shared: &mut SharedContext) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)>
    {
        let guard = shared.broker.lock_arc().await;
        let channels = guard.get_channels();
        std::mem::drop(guard);
        let message = ProtocolPackage::InfoListChannelsReply{ channels };

        async_with_context!(self.socket.send_package(message).await, self);

        Some((self.into(), Reaction::Success))
    }

    // TODO: join channel notification
    async fn join_channel(mut self, shared: &mut SharedContext, channel: String) -> Option<(ClientConnectionAuthenticatedEdges, Reaction)> 
    {
        let mut guard = shared.broker.lock_arc().await;
        let res = guard.subscribe(channel.clone(), self.socket.clone());
        if let Err(error) = res {
            let message = ProtocolPackage::Deny{ error: error.clone() };
            async_with_context!(self.socket.send_package(message).await, self);
            with_context!(Err(error), self);
        }
        let new_state = ClientChannelConnection {
            broker: self.broker,
            socket: self.socket,
            username: self.username,
            channel,
        };
        Some((new_state.into(), Reaction::Success))
    }

    pub async fn shutdown(self) {
        let mut guard = self.broker.lock_arc().await;
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
        state.shutdown(false);
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
    async fn send_message(self, shared: &mut SharedContext, message: String) -> Option<(ClientChannelConnectionEdges, Reaction)> {
        let message = ProtocolPackage::ChatMessageReceive { username: self.username.clone(), message };
        let mut guard = shared.broker.lock_arc().await;
        with_context!(guard.notify(&self.channel, message).await, self);
        Some((self.into(), Reaction::Success))
    } 

    pub async fn shutdown(self, intentional: bool) {
        let mut guard = self.broker.lock_arc().await;
        guard.unsubscribe(&self.channel, &self.socket);
        guard.deregister_username(&self.username);

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
}

#[::rust_state_machine::async_trait::async_trait]
impl AsyncProgress<ClientChannelConnectionEdges, ServerSideConnectionSM> for ClientChannelConnection {
    async fn transition(mut self, shared: &mut SharedContext, _: ()) -> Option<(ClientChannelConnectionEdges, Reaction)> {
        let input = async_with_context!(self.socket.receive_package().await, self);
        match input {
            ProtocolPackage::ChatMessageSend{ message } => self.send_message(shared, message).await, 
            ProtocolPackage::ChannelDisconnectNotification => {
                let mut guard = shared.broker.lock_arc().await;
                guard.unsubscribe(&self.channel, &self.socket);
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


// ### EDGES ###
// #[derive(Debug, Clone)]
// pub enum ClientConnectionEdges<'a> {
//     Disconnected,
//     Authenticated(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
// }
//
// #[derive(Debug, Clone)]
// pub enum ClientConnectionAuthenticatedEdges<'a> {
//     Disconnected,
//     ChannelConnect(ServerSideConnectionFMS<'a, ClientChannelConnection>),
//     ListChannels(ServerSideConnectionFMS<'a, ClientConnectionAuthenticated>),
// }
//
// #[derive(Debug, Clone)]
// pub struct ServerSideConnectionFMS<'a, S: Clone> {
//     broker: &'a Arc<Mutex<Broker>>,
//     state: S,
// }
//
// impl<'a> ServerSideConnectionFMS<'a, ClientConnection> {
//     pub fn new(broker: &'a Arc<Mutex<Broker>>, connection: TcpStream) -> Self {
//         ServerSideConnectionFMS {
//             broker,
//             state: ClientConnection {
//                 socket: connection
//             }
//         }
//     }
//
//     pub async fn authenticate(mut self, username: String) -> SResult<'a, ClientConnectionAuthenticated, ClientConnection> {
//         let mut guard = self.broker.lock_arc().await;               
//         guard.register_username(username.clone())
//             .map_err(|e| {
//                 ServerFailEdges::from_errortype(self.clone(), e)
//             })?;
//
//         // TODO remove clone, self should be able to be given as value, 
//         //but rust cannot infer that self won't be used after this statement
//
//         std::mem::drop(guard); // not strictly needed but the faster the mutex guard is dropped the better 
//
//         let reply = ProtocolPackage::ServerConnectionAccept;
//         let temp: ServerResult<'a, (), ClientConnection> = self.state.socket.send_package(reply).await;
//         temp?;
//
//         Ok(ServerSideConnectionFMS {
//             broker: self.broker,
//             state: ClientConnectionAuthenticated {
//                 socket: self.state.socket,
//                 username,
//             }
//         })
//     }
//
//     pub async fn disconnect(self) -> ServerResult<'a, (), ClientConnection> {
//         self.state.socket.shutdown(std::net::Shutdown::Both)?;
//         Ok(())
//     }
//
//     pub async fn listen(mut self) -> ServerResult<'a, ClientConnectionEdges<'a>, ClientConnection> {
//         let package: ServerResult<'a, ProtocolPackage, ClientConnection> = self.state.socket.receive_package().await;
//         let package = package?;
//         use crate::protocol::ProtocolPackage::*;
//         use ClientConnectionEdges::*;
//         match package {
//             ServerConnectionRequest { username } => Ok(Authenticated(self.authenticate(username).await?)), 
//             DisconnectNotification => {
//                 self.disconnect().await?;
//                 Ok(Disconnected)
//             }, 
//             _ => Err(ServerFailEdges::MalformedPackage(self)) 
//         }
//     }
// }
//
//
// impl<'a> ServerSideConnectionFMS<'a, ClientConnectionAuthenticated> {
//     pub async fn disconnect(self) -> ServerResult<'a, (), ClientConnectionAuthenticated> {
//         let mut guard = self.broker.lock_arc().await;               
//         guard.deregister_username(&self.state.username);
//         self.state.socket.shutdown(std::net::Shutdown::Both)?;
//         Ok(())
//     }
// }
//
// impl<'a, E, T: HasServerConnection<E> + Clone> HasServerConnection<E> for ServerSideConnectionFMS<'a, T> 
//     where E: From<Box<bincode::ErrorKind>> + From<std::io::Error> {
//     fn get_server_socket(&mut self) ->  &mut TcpStream {
//         self.state.get_server_socket()
//     }
// }

