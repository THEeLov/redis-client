//! RESP2 encoding of commands and decoding of replies.

use std::fmt;
use std::io::{self, BufRead};

/// One reply from the server.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Vec<u8>),
    Null,
}

impl Reply {
    /// A protocol error means the server is about to close the connection.
    #[must_use]
    pub fn is_protocol_error(&self) -> bool {
        matches!(self, Self::Error(msg) if msg.starts_with("ERR Protocol error"))
    }
}

/// Formats a reply the way `redis-cli` does.
impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple(s) => write!(f, "{s}"),
            Self::Error(e) => write!(f, "(error) {e}"),
            Self::Integer(i) => write!(f, "(integer) {i}"),
            Self::Null => write!(f, "(nil)"),
            Self::Bulk(data) => {
                write!(f, "\"")?;
                for &b in data {
                    match b {
                        b'"' => write!(f, "\\\"")?,
                        b'\\' => write!(f, "\\\\")?,
                        b'\n' => write!(f, "\\n")?,
                        b'\r' => write!(f, "\\r")?,
                        b'\t' => write!(f, "\\t")?,
                        0x20..=0x7e => write!(f, "{}", b as char)?,
                        _ => write!(f, "\\x{b:02x}")?,
                    }
                }
                write!(f, "\"")
            }
        }
    }
}

/// Encodes a command as a RESP array of bulk strings.
pub fn encode_command<A: AsRef<[u8]>>(args: &[A]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        let arg = arg.as_ref();
        out.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        out.extend_from_slice(arg);
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// Reads exactly one reply. Bulk data is read by its length, never by
/// searching for `\r\n`.
pub fn read_reply(reader: &mut impl BufRead) -> io::Result<Reply> {
    let mut line = Vec::new();
    if reader.read_until(b'\n', &mut line)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let line = line
        .strip_suffix(b"\r\n")
        .ok_or_else(|| invalid("reply line not terminated by CRLF"))?;
    let (&kind, rest) = line.split_first().ok_or_else(|| invalid("empty reply line"))?;
    let rest = String::from_utf8_lossy(rest);

    Ok(match kind {
        b'+' => Reply::Simple(rest.into_owned()),
        b'-' => Reply::Error(rest.into_owned()),
        b':' => Reply::Integer(rest.parse().map_err(|_| invalid("bad integer"))?),
        b'$' => match rest.parse::<i64>().map_err(|_| invalid("bad bulk length"))? {
            -1 => Reply::Null,
            len => {
                let len = usize::try_from(len).map_err(|_| invalid("bad bulk length"))?;
                let mut data = vec![0; len + 2]; // data + "\r\n"
                reader.read_exact(&mut data)?;
                if !data.ends_with(b"\r\n") {
                    return Err(invalid("bulk string not terminated by CRLF"));
                }
                data.truncate(len);
                Reply::Bulk(data)
            }
        },
        other => return Err(invalid(&format!("unknown reply type {:?}", other as char))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encodes_command() {
        assert_eq!(
            encode_command(&["SET", "greeting", "hello world"]),
            b"*3\r\n$3\r\nSET\r\n$8\r\ngreeting\r\n$11\r\nhello world\r\n"
        );
        // Lengths are in bytes, not characters.
        assert_eq!(encode_command(&["čau"]), "*1\r\n$4\r\nčau\r\n".as_bytes());
    }

    #[test]
    fn reads_all_reply_types_from_one_buffer() {
        let mut r = Cursor::new(&b"+OK\r\n-ERR syntax error\r\n:-2\r\n$4\r\na\r\nb\r\n$0\r\n\r\n$-1\r\n"[..]);
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Simple("OK".into()));
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Error("ERR syntax error".into()));
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Integer(-2));
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Bulk(b"a\r\nb".to_vec()));
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Bulk(Vec::new()));
        assert_eq!(read_reply(&mut r).unwrap(), Reply::Null);
        assert_eq!(read_reply(&mut r).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn rejects_malformed_replies() {
        for bad in [&b"?x\r\n"[..], b"\r\n", b":abc\r\n", b"$-5\r\n", b"$2\r\nabcd", b"+OK\n"] {
            assert!(read_reply(&mut Cursor::new(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn detects_protocol_error() {
        assert!(Reply::Error("ERR Protocol error: invalid bulk length".into()).is_protocol_error());
        assert!(!Reply::Error("ERR syntax error".into()).is_protocol_error());
    }

    #[test]
    fn displays_like_redis_cli() {
        assert_eq!(Reply::Bulk(b"a \"b\"\n\x01".to_vec()).to_string(), r#""a \"b\"\n\x01""#);
        assert_eq!(Reply::Integer(3).to_string(), "(integer) 3");
        assert_eq!(Reply::Null.to_string(), "(nil)");
    }
}
