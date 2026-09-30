# rust-redis-client

A small, dependency-free Rust library for talking to a Redis-compatible server
over a Unix socket. It speaks the RESP2 protocol and gives you typed methods
such as `set`, `get` and `incr` instead of raw protocol messages.

```rust
use rust_redis_client::{Client, Commands};

fn main() -> rust_redis_client::Result<()> {
    let mut client = Client::connect("/tmp/myredis.sock")?;

    client.set("A", 1)?;
    assert_eq!(client.incr("A")?, 2);
    assert_eq!(client.get("A")?, Some(b"2".to_vec()));

    Ok(())
}
```

## Installation

```sh
cargo add rust-redis-client
```

or add it to your `Cargo.toml` by hand:

```toml
[dependencies]
rust-redis-client = "0.1"
```

It needs Rust 1.85 or newer (edition 2024) and a Unix-like OS, because it
connects through `std::os::unix::net::UnixStream`.

## Connecting

```rust
use std::time::Duration;
use rust_redis_client::Client;

// Uses the default timeout of 5 seconds for every read and write.
let mut client = Client::connect("/tmp/myredis.sock")?;

// Or pick your own timeout.
let mut client = Client::connect_with_timeout("/tmp/myredis.sock", Duration::from_millis(500))?;
```

`connect` also sends a `PING` and waits for `PONG`. A socket can accept
connections even when the server behind it is stuck (for example suspended
with Ctrl+Z), so a successful `connect` means the server is really answering.

## Commands

All commands are methods of the `Commands` trait, so bring it into scope with
`use rust_redis_client::Commands`.

| Method                 | Redis command    | Returns                              |
| ---------------------- | ---------------- | ------------------------------------ |
| `ping()`               | `PING`           | `()`                                 |
| `ping_message(msg)`    | `PING message`   | `Vec<u8>`, the message back          |
| `echo(message)`        | `ECHO`           | `Vec<u8>`                            |
| `set(key, value)`      | `SET`            | `()`                                 |
| `get(key)`             | `GET`            | `Option<Vec<u8>>`, `None` if missing |
| `del(key)`             | `DEL`            | `bool`, whether the key existed      |
| `del_many(keys)`       | `DEL key ...`    | `u64`, how many keys existed         |
| `exists(key)`          | `EXISTS`         | `bool`                               |
| `exists_many(keys)`    | `EXISTS key ...` | `u64`, a key listed twice counts twice |
| `incr(key)`            | `INCR`           | `i64`, the new value                 |
| `decr(key)`            | `DECR`           | `i64`, the new value                 |

Every method returns `rust_redis_client::Result<T>`.

### Arguments

Keys and values accept anything that implements `ToArg`: `&str`, `String`,
`&[u8]`, `Vec<u8>`, byte arrays, and all integer and float types. Numbers are
sent in decimal form, so `client.set("A", 1)` sends `SET A 1`.

Values come back as raw bytes (`Vec<u8>`), because the server may store
binary data. Convert them yourself when you expect text:

```rust
if let Some(bytes) = client.get("greeting")? {
    println!("{}", String::from_utf8_lossy(&bytes));
}
```

### Raw commands

To send a command that has no method yet, use `request`. It returns the
server's raw `Reply`:

```rust
use rust_redis_client::{Commands, Reply, ToArg};

let reply = client.request(&["EXPIRE".to_arg(), "A".to_arg(), 10.to_arg()])?;
match reply {
    Reply::Integer(1) => println!("timeout set"),
    Reply::Error(msg) => eprintln!("server error: {msg}"),
    other => println!("{other}"),
}
```

`Reply` implements `Display` and prints replies the way `redis-cli` does, for
example `(integer) 1`, `(nil)` or `"hello"`.

## Error handling

`rust_redis_client::Error` has three variants:

| Variant                  | Meaning                                                                                                          |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| `Error::Io(io::Error)`   | The connection failed, timed out or the server sent malformed data. Drop the client and connect again. |
| `Error::Server(String)`  | The server answered with an error, for example `ERR value is not an integer`. The connection is still usable. |
| `Error::UnexpectedReply(Reply)` | The server answered with a reply type the command does not expect.                                        |

```rust
use rust_redis_client::{Commands, Error};

match client.incr("name") {
    Ok(n) => println!("new value: {n}"),
    Err(Error::Server(msg)) => eprintln!("server refused: {msg}"),
    Err(e) => return Err(e),
}
```

`Error` implements `std::error::Error`, so it also works with `?`, `Box<dyn Error>`
and crates like `anyhow`.

`request` is the one exception to the table above: it returns error replies
as `Ok(Reply::Error(..))` so you can inspect them yourself. After a protocol
error the server closes the connection, and `reply.is_protocol_error()` tells
you when that has happened.

## Implementing `Commands` yourself

`Commands` has a single required method, `request`. All the other methods
have default implementations built on top of it. Any type that can send a
command and return a `Reply` gets the full API, which is handy for testing
code without a running server:

```rust
use rust_redis_client::{Commands, Reply, Result};

struct AlwaysOk;

impl Commands for AlwaysOk {
    fn request(&mut self, _args: &[Vec<u8>]) -> Result<Reply> {
        Ok(Reply::Simple("OK".into()))
    }
}

AlwaysOk.set("A", 1).unwrap();
```

### Adding a new command

To add a typed method, add a default method to the `Commands` trait in
[`src/commands.rs`](src/commands.rs). It then works for every client:

```rust
/// Sets a timeout of `seconds` on `key`. Returns whether the key exists.
fn expire(&mut self, key: impl ToArg, seconds: u64) -> Result<bool> {
    integer(call(self, &[b"EXPIRE".to_arg(), key.to_arg(), seconds.to_arg()])?).map(|n| n > 0)
}
```

## Limitations

- Only Unix sockets are supported, not TCP.
- RESP2 replies of type simple string, error, integer and bulk string are
  supported. Arrays, which commands like `KEYS` or `MGET` return, are not yet.
- One `Client` is one connection, used for one request at a time. No
  pipelining or connection pooling.
- `Client` doesn't reconnect by itself. After an `Error::Io`, create a new one.

## Project layout

```
src/
├── lib.rs       public API and re-exports
├── client.rs    Client: the socket connection and timeouts
├── commands.rs  Commands trait with the typed command methods
├── arg.rs       ToArg: converting Rust values into command arguments
├── error.rs     Error and Result
└── resp.rs      RESP encoding and decoding, Reply
```

## Development

```sh
cargo test     # unit tests and doc tests, no server needed
cargo clippy --all-targets -- -W clippy::pedantic -W clippy::nursery
cargo doc --open
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
