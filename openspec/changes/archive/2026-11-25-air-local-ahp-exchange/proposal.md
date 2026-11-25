# Proposal

## Why

Air removes its local launch socket. A local app now speaks local AHP, the protocol of `air-ide`: the local peer
handshake v2 with `initialize` and no `access`, then JSON-RPC of `ahp-root://`. The old socket gives the session a
provider id, which the rows of the IDE do not use, so a watch of a terminal agent ended with `NO_SESSION`.

## What Changes

- The discovery reads the instance files of schema 2 in `JetBrains/air/ahp`, and asks each IDE `x-air-ij/hosts`.
- The client sends `initialize` in the envelope, then requests with ids, and reads the answer of each id; a notice
  waits for the watch. An error of the IDE is a refusal with the code of `error.data.code`.
- Run is `createSession` and Draft is `x-air-ij/openDraft`; a launch without a target starts the default of the New
  Session button of the IDE.
- A watch subscribes the root, lists the rows with `listSessions`, and reads the root notices.

Capabilities: `branch/start-in-air`.
