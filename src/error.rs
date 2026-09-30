use std::fmt;
use std::io;

use crate::resp::Reply;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// The connection failed, timed out or the server sent malformed data.
    /// The connection should not be used any more.
    Io(io::Error),
    /// The server answered with an error reply, e.g. `ERR syntax error`.
    Server(String),
    /// The server answered with a reply of a type the command does not expect.
    UnexpectedReply(Reply),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Server(msg) => write!(f, "{msg}"),
            Self::UnexpectedReply(reply) => write!(f, "unexpected reply: {reply}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
