# Tasks

- [x] The channel on tokio-rustls and a `Framed` of `LengthDelimitedCodec`, its deadlines by `tokio::time::timeout`
- [x] The relay as the pipes of `nc`, one stream for TLS
- [x] The client async: the steps of a run as futures of one task, the stops by tokens, git on the blocking pool
- [x] The agent on a multi-thread runtime: a task per connection, the follow reader on the blocking pool
- [x] The tests of the channel and of the client on paused time over pipes in memory (`MemoryNet`)
- [x] docs/design/host-runs.md and docs/contributing/rust-tests.md name the runtime and the tests on paused time
