use crate::arg::ToArg;
use crate::error::{Error, Result};
use crate::resp::Reply;

/// The commands a client can send to the server.
///
/// An implementor only provides [`request`](Commands::request), which sends
/// one raw command and returns the server's reply. Every other method is built
/// on top of it, turning error replies into [`Error::Server`] and the reply
/// into a convenient Rust type.
///
/// # Errors
///
/// Every method fails with
/// - [`Error::Io`] if the connection breaks, times out or the server sends
///   malformed data; the connection should not be used any more,
/// - [`Error::Server`] if the server answers with an error reply,
/// - [`Error::UnexpectedReply`] if the reply has a type the command does not
///   expect.
pub trait Commands {
    /// Sends one command and returns the raw reply. Error replies from the
    /// server are returned as `Ok(Reply::Error(..))`, not as `Err`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the connection breaks, times out or the server
    /// sends malformed data.
    fn request(&mut self, args: &[Vec<u8>]) -> Result<Reply>;

    /// Checks that the server is responsive.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn ping(&mut self) -> Result<()> {
        match call(self, &[b"PING".to_arg()])? {
            Reply::Simple(s) if s == "PONG" => Ok(()),
            other => Err(Error::UnexpectedReply(other)),
        }
    }

    /// Sends a `PING` with `message`, which the server returns back.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn ping_message(&mut self, message: impl ToArg) -> Result<Vec<u8>> {
        bulk(call(self, &[b"PING".to_arg(), message.to_arg()])?)
    }

    /// Returns `message` back from the server.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn echo(&mut self, message: impl ToArg) -> Result<Vec<u8>> {
        bulk(call(self, &[b"ECHO".to_arg(), message.to_arg()])?)
    }

    /// Stores `value` under `key`, replacing any previous value.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn set(&mut self, key: impl ToArg, value: impl ToArg) -> Result<()> {
        match call(self, &[b"SET".to_arg(), key.to_arg(), value.to_arg()])? {
            Reply::Simple(s) if s == "OK" => Ok(()),
            other => Err(Error::UnexpectedReply(other)),
        }
    }

    /// Returns the value stored under `key`, or `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn get(&mut self, key: impl ToArg) -> Result<Option<Vec<u8>>> {
        match call(self, &[b"GET".to_arg(), key.to_arg()])? {
            Reply::Null => Ok(None),
            reply => bulk(reply).map(Some),
        }
    }

    /// Deletes `key`. Returns whether it existed.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn del(&mut self, key: impl ToArg) -> Result<bool> {
        integer(call(self, &[b"DEL".to_arg(), key.to_arg()])?).map(|n| n > 0)
    }

    /// Deletes all `keys`. Returns how many of them existed.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors). The server answers with an error
    /// if `keys` is empty.
    fn del_many<K: ToArg>(&mut self, keys: impl IntoIterator<Item = K>) -> Result<u64> {
        count(call(self, &with_keys(b"DEL", keys))?)
    }

    /// Returns whether `key` exists.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors).
    fn exists(&mut self, key: impl ToArg) -> Result<bool> {
        integer(call(self, &[b"EXISTS".to_arg(), key.to_arg()])?).map(|n| n > 0)
    }

    /// Returns how many of `keys` exist. A key listed twice counts twice.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors). The server answers with an error
    /// if `keys` is empty.
    fn exists_many<K: ToArg>(&mut self, keys: impl IntoIterator<Item = K>) -> Result<u64> {
        count(call(self, &with_keys(b"EXISTS", keys))?)
    }

    /// Increments the number stored under `key` by one and returns the new
    /// value. A missing key counts as 0.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors). The server answers with an error
    /// if the stored value is not an integer.
    fn incr(&mut self, key: impl ToArg) -> Result<i64> {
        integer(call(self, &[b"INCR".to_arg(), key.to_arg()])?)
    }

    /// Decrements the number stored under `key` by one and returns the new
    /// value. A missing key counts as 0.
    ///
    /// # Errors
    ///
    /// See [the trait docs](Commands#errors). The server answers with an error
    /// if the stored value is not an integer.
    fn decr(&mut self, key: impl ToArg) -> Result<i64> {
        integer(call(self, &[b"DECR".to_arg(), key.to_arg()])?)
    }
}

/// Sends a command and turns an error reply into `Err`.
fn call<C: Commands + ?Sized>(client: &mut C, args: &[Vec<u8>]) -> Result<Reply> {
    match client.request(args)? {
        Reply::Error(msg) => Err(Error::Server(msg)),
        reply => Ok(reply),
    }
}

fn bulk(reply: Reply) -> Result<Vec<u8>> {
    match reply {
        Reply::Bulk(data) => Ok(data),
        other => Err(Error::UnexpectedReply(other)),
    }
}

fn integer(reply: Reply) -> Result<i64> {
    match reply {
        Reply::Integer(n) => Ok(n),
        other => Err(Error::UnexpectedReply(other)),
    }
}

/// Builds the arguments of a command that takes a list of keys.
fn with_keys<K: ToArg>(name: &[u8], keys: impl IntoIterator<Item = K>) -> Vec<Vec<u8>> {
    std::iter::once(name.to_vec())
        .chain(keys.into_iter().map(|key| key.to_arg()))
        .collect()
}

fn count(reply: Reply) -> Result<u64> {
    match reply {
        Reply::Integer(n) => u64::try_from(n).map_err(|_| Error::UnexpectedReply(reply)),
        other => Err(Error::UnexpectedReply(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Records the commands it gets and answers with prepared replies.
    #[derive(Default)]
    struct Fake {
        sent: Vec<Vec<Vec<u8>>>,
        replies: VecDeque<Reply>,
    }

    impl Fake {
        fn answering(replies: impl IntoIterator<Item = Reply>) -> Self {
            Self { sent: Vec::new(), replies: replies.into_iter().collect() }
        }
    }

    impl Commands for Fake {
        fn request(&mut self, args: &[Vec<u8>]) -> Result<Reply> {
            self.sent.push(args.to_vec());
            Ok(self.replies.pop_front().expect("no reply prepared"))
        }
    }

    #[test]
    fn sends_arguments_as_bytes() {
        let mut fake = Fake::answering([Reply::Simple("OK".into())]);
        fake.set("A", 1).unwrap();
        assert_eq!(fake.sent, [[b"SET".to_vec(), b"A".to_vec(), b"1".to_vec()]]);
    }

    #[test]
    fn converts_replies() {
        let mut fake = Fake::answering([
            Reply::Simple("PONG".into()),
            Reply::Bulk(b"v".to_vec()),
            Reply::Null,
            Reply::Integer(1),
            Reply::Integer(0),
            Reply::Integer(7),
        ]);
        fake.ping().unwrap();
        assert_eq!(fake.get("k").unwrap(), Some(b"v".to_vec()));
        assert_eq!(fake.get("missing").unwrap(), None);
        assert!(fake.del("k").unwrap());
        assert!(!fake.exists("k").unwrap());
        assert_eq!(fake.incr("n").unwrap(), 7);
    }

    #[test]
    fn sends_every_key() {
        let mut fake = Fake::answering([Reply::Integer(2), Reply::Integer(1)]);
        assert_eq!(fake.exists_many(["a", "a", "x"]).unwrap(), 2);
        assert_eq!(fake.del_many(vec![b"a".to_vec()]).unwrap(), 1);
        assert_eq!(fake.sent[0], [b"EXISTS".to_vec(), b"a".to_vec(), b"a".to_vec(), b"x".to_vec()]);
        assert_eq!(fake.sent[1], [b"DEL".to_vec(), b"a".to_vec()]);
    }

    #[test]
    fn ping_with_message_returns_it() {
        let mut fake = Fake::answering([Reply::Bulk(b"hi".to_vec())]);
        assert_eq!(fake.ping_message("hi").unwrap(), b"hi");
        assert_eq!(fake.sent, [[b"PING".to_vec(), b"hi".to_vec()]]);
    }

    #[test]
    fn reports_server_errors_and_unexpected_replies() {
        let mut fake = Fake::answering([
            Reply::Error("ERR value is not an integer".into()),
            Reply::Bulk(b"oops".to_vec()),
        ]);
        assert!(matches!(fake.incr("k"), Err(Error::Server(msg)) if msg.contains("integer")));
        assert!(matches!(fake.set("k", "v"), Err(Error::UnexpectedReply(_))));
    }
}
