use std::{error::Error, fmt::Debug};
use thiserror::Error;

use crate::protocol::server::ServerSideConnectionFMS;


#[derive(Debug, Error)]
pub enum ErrorType {
    #[error("Username already exists")]
    UsernameAlreadyExists,
}

#[derive(Debug, Error)]
pub enum FailEdges<'a, T: Debug + Clone> {
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


pub type Result<'a, SuccessState, FailState> = std::result::Result<SuccessState, FailEdges<'a, FailState>>;

pub type SResult<'a, SuccessState, FailState> = Result<
    'a,
    ServerSideConnectionFMS<'a, SuccessState>, 
    FailState
>;

pub type GResult<T> = std::result::Result<T, Box<dyn Error>>;

impl<'a, T: Debug + Clone> FailEdges<'a, T> {
    pub fn from_errortype(ssc_fsm: ServerSideConnectionFMS<'a, T>, error_type: ErrorType) -> FailEdges<'a, T> {
        FailEdges::Rejected(ssc_fsm, error_type)
    }
}

