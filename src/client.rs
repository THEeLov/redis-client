use std::io::{self, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::commands::Commands;
use crate::error::Result;
use crate::resp::{Reply, encode_command, read_reply};

/// A connection to the server. Commands and replies are strictly paired,
/// so one request is followed by reading exactly one reply.
///
/// The methods for sending commands come from the [`Commands`] trait.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    timeout: Duration,
}

impl Client {
    /// How long to wait for the server before giving up on a reply.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Connects to the server listening on the Unix socket at `path`.
    ///
    /// # Errors
    ///
    /// See [`connect_with_timeout`](Client::connect_with_timeout).
    pub fn connect(path: impl AsRef<Path>) -> Result<Self> {
        Self::connect_with_timeout(path, Self::DEFAULT_TIMEOUT)
    }

    /// Like [`connect`](Client::connect), but waits at most `timeout` for
    /// each read or write.
    ///
    /// Connecting also checks with a `PING` that the server really answers.
    /// A socket can accept connections even when the server behind it is
    /// stuck (for example suspended with Ctrl+Z), so `connect` alone is not
    /// enough.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`](crate::Error::Io) if the socket cannot be opened or the server does
    /// not answer the `PING` in time, and [`Error::UnexpectedReply`](crate::Error::UnexpectedReply) if it
    /// answers something other than `PONG`.
    pub fn connect_with_timeout(path: impl AsRef<Path>, timeout: Duration) -> Result<Self> {
        let writer = UnixStream::connect(path)?;
        writer.set_read_timeout(Some(timeout))?;
        writer.set_write_timeout(Some(timeout))?;
        let reader = BufReader::new(writer.try_clone()?);
        let mut client = Self {
            reader,
            writer,
            timeout,
        };
        client.ping()?;
        Ok(client)
    }

    fn describe(&self, e: io::Error) -> io::Error {
        match e.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => io::Error::new(
                e.kind(),
                format!("server did not respond within {:?}", self.timeout),
            ),
            io::ErrorKind::UnexpectedEof => {
                io::Error::new(e.kind(), "server closed the connection")
            }
            _ => e,
        }
    }
}

impl Commands for Client {
    fn request(&mut self, args: &[Vec<u8>]) -> Result<Reply> {
        self.writer
            .write_all(&encode_command(args))
            .map_err(|e| self.describe(e))?;
        Ok(read_reply(&mut self.reader).map_err(|e| self.describe(e))?)
    }
}
