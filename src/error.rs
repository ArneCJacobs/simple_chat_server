use std::{error::Error, fmt::{Display, Debug}};
use thiserror::Error;

use crate::protocol::server::ServerSideConnectionFMS;


#[derive(Debug, Error)]
pub enum ErrorType {
    #[error("Username already exists")]
    UsernameAlreadyExists,
}

#[derive(Debug, Error)]
pub enum FailEdges<'a, T: Debug> {
    #[error("Tcp connection ended")]
    Disconnected,

    #[error("The request was rejected because {1}")]
    Rejected(ServerSideConnectionFMS<'a, T>, ErrorType),

    #[error("Received a malformed or unexpected package")]
    MalformedPackage(ServerSideConnectionFMS<'a, T>),

    #[error(transparent)]
    InternalProcessError(#[from] Box<dyn std::error::Error>)
}


pub type Result<'a, N, T> = std::result::Result<N, FailEdges<'a, T>>;
pub type SResult<'a, N, T> = Result<'a,ServerSideConnectionFMS<'a, N>, ServerSideConnectionFMS<'a, T>>;
pub type GResult<T> = std::result::Result<T, Box<dyn Error>>;

impl<'a, T: Debug> From<(ServerSideConnectionFMS<'a, T>, ErrorType)> for FailEdges<'a, T> {
    fn from(pair: (ServerSideConnectionFMS<'a, T>, ErrorType)) -> Self {
        let (context, error) = pair;
        FailEdges::Rejected(context, error)
    }
}

