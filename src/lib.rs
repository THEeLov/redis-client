//! A small client for a Redis-compatible server listening on a Unix socket.
//!
//! ```no_run
//! use redis_client::{Client, Commands};
//!
//! let mut client = Client::connect("/tmp/myredis.sock")?;
//! client.set("A", 1)?;
//! assert_eq!(client.incr("A")?, 2);
//! assert_eq!(client.get("A")?, Some(b"2".to_vec()));
//! # Ok::<(), redis_client::Error>(())
//! ```

mod arg;
mod client;
mod commands;
mod error;
mod resp;

pub use arg::ToArg;
pub use client::Client;
pub use commands::Commands;
pub use error::{Error, Result};
pub use resp::Reply;
