# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-09-30

### Added

- `Commands::ping_message` sends `PING` with a message and returns the message.
- `Commands::del_many` deletes several keys and returns how many of them existed.
- `Commands::exists_many` returns how many of the given keys exist. A key
  listed twice counts twice.

## [0.1.0] - 2026-09-30

### Added

- `Client` that connects to a Redis-compatible server over a Unix socket and
  speaks RESP2, with a configurable read/write timeout.
- `Commands` trait with `ping`, `echo`, `set`, `get`, `del`, `exists`, `incr`
  and `decr`, plus `request` for sending any other command.
- `Reply`, `Error` and `ToArg` types.
