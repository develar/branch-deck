# Proposal

## Why

The link of `brd-host` was a blocking stack of its own: a hand-written length prefix over a blocking rustls stream,
each deadline a socket timeout (`SO_RCVTIMEO`), a closer that shut a socket down from another thread, and a relay that
copied between a loopback socket and the pipes of `/usr/bin/nc` on two threads. Its tests raced real sleeps against
real deadlines, and two of them failed under load on 2026-11-23. A deadline of the socket does not reach the pipes of
the relay at all.

## What Changes

- The channel runs on tokio: TLS by tokio-rustls on the same rustls 0.23 and provider, the frames by a `Framed` of
  `LengthDelimitedCodec`, each read and write under `tokio::time::timeout`. The bytes on the wire stay as they were, so
  the schema stays 1 and an agent of an older build takes the next binary over the link.
- The relay is the stdin and the stdout of `nc` as one stream; the drop of the connection ends `nc`.
- The client and the agent run on tokio. The steps of a run that go on beside each other are futures of one task, a
  step stops another by a token or by the drop of its future, and git runs on the blocking pool. The agent serves a
  connection per task, its blocking work on the blocking pool, and sends the heartbeats meanwhile.
- Heartbeats stay messages of the protocol: the relay carries no keepalive of TCP, and a quiet run must differ from a
  dead link.
- The tests of the channel and of the client run on paused time over pipes in memory, so no test waits for the clock.

Capabilities: `cli/host`.
