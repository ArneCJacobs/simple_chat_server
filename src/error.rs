use std::{error::Error, fmt::Debug};
use thiserror::Error;

use crate::protocol::{server::ServerSideConnectionFMS, client::{ClientSideConnectionFMS, NotConnected}};
use std::result::Result as StdResult;


#[derive(Debug, Error)]
pub enum ErrorType {
    #[error("Username already exists")]
    UsernameAlreadyExists,

    #[error("Failed to connect")]
    FailedToConnect,
}

#[derive(Debug, Error)]
pub enum ServerFailEdges<'a, T: Debug + Clone> {
    #[error("Tcp connection ended")]
    Disconnected,

    #[error("The request was rejected because {1}")]
    Rejected(ServerSideConnectionFMS<'a, T>, ErrorType),

    #[error("Received a malformed or unexpected package")]
    MalformedPackage(ServerSideConnectionFMS<'a, T>),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    DeserializeError(#[from] Box<bincode::ErrorKind>),

}

#[derive(Debug, Error)]
pub enum ClientFailEdges<T: Debug + Clone> {
    #[error("Tcp connection ended")]
    Disconnected(ClientSideConnectionFMS<NotConnected>),

    #[error("The request was rejected because {1}")]
    Rejected(ClientSideConnectionFMS< T>, ErrorType),

    #[error("Received a malformed or unexpected package")]
    MalformedPackage(ClientSideConnectionFMS<T>),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    DeserializeError(#[from] Box<bincode::ErrorKind>),

}


pub type ServerResult<'a, SuccessResult, FailState> = StdResult<SuccessResult, ServerFailEdges<'a, FailState>>;
pub type SResult<'a, SuccessState, FailState> = StdResult<
    ServerSideConnectionFMS<'a, SuccessState>, 
    ServerFailEdges<'a, FailState> 
>;

pub type ClientResult<SuccessResult, FailState> = StdResult<SuccessResult, ClientFailEdges<FailState>>;
pub type CResult<SuccessState, FailState> = StdResult<ClientSideConnectionFMS<SuccessState>, ClientFailEdges<FailState>>;


pub type GResult<T> = std::result::Result<T, Box<dyn Error>>;

impl<'a, T: Debug + Clone> ServerFailEdges<'a, T> {
    pub fn from_errortype(ssc_fsm: ServerSideConnectionFMS<'a, T>, error_type: ErrorType) -> ServerFailEdges<'a, T> {
        ServerFailEdges::Rejected(ssc_fsm, error_type)
    }
}

impl<'a, T: Debug + Clone> ClientFailEdges<T> {
    pub fn from_errortype(csc_fsm: ClientSideConnectionFMS<T>, error_type: ErrorType) -> ClientFailEdges<T> {
        ClientFailEdges::Rejected(csc_fsm, error_type)
    }
}


impl<'a, T: Debug + Clone + Send + Sync> From<ServerFailEdges<'a, T>> for std::io::Error {
    fn from(_: ServerFailEdges<'a, T>) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, "Welp, whoops")
    }
}
