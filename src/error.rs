use std::{error::Error, fmt::Display};

use crate::protocol::server::ServerSideConnectionFMS;

#[derive(Debug)]
pub enum FailEdges<'a, T, E: Error> {
    Disconnected,
    Rejected(ServerSideConnectionFMS<'a, T>, E),
    MalformedPackage(ServerSideConnectionFMS<'a, T>),
}

impl<'a, T, E: Error> Display for FailEdges<'a, T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Disconnected =>  write!(f, "Tcp connection closed"),
            Self::Rejected(_, error) => write!(f, "Rejected due to following error: {:}", error),
            Self::MalformedPackage(_) => write!(f, "Malformed/Unexpected package received")
        }
    }
}

impl<'a, T, E: Error + 'static> Error for FailEdges<'a, T, E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            Self::Disconnected => None,
            Self::MalformedPackage(_) => None,
            Self::Rejected(_,ref error) => Some(error),
        }
    }
}

pub type Result<'a, T> = std::result::Result<T, FailEdges<'a, T, Box<dyn Error>>>;
pub type SResult<'a, T> = Result<'a, ServerSideConnectionFMS<'a, T>>;
