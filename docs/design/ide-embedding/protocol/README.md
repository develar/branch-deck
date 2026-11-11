# The contract of air-apps

The OpenSpec capability `air-apps-host` (`plugins/air/openspec`) states the rules, and the decision record
`plugins/air/docs/decisions/2026-11-11-the-ide-hosts-an-app-of-the-user-over-x-air-app.md` states why.
These files pin the wire.

| File | What it pins |
|---|---|
| `air-app.schema.json` | The app file `~/.air/apps/<id>.yaml`, format 1, with the defaults that the registry fills in |
| `examples/branch-deck.yaml` | The app file that the installer of Branch Deck writes |
| `x-air-app.schema.json` | One line of the corpus. The defs `AppMessage`, `PageMessage`, `FrameMessage` and `IdeMessage` are the messages of the four links |
| `corpus/*.ndjson` | The golden frames, one JSON object per line |

## The links

| `link` | Ends (`from`) | Wire |
|---|---|---|
| `app` | `air-ide`, `app` | The stdio of the app process: MCP JSON-RPC 2.0, one object per line |
| `page` | `page`, `air-ide` | The WebSocket of the host page on `127.0.0.1`: one JSON-RPC 2.0 object per text frame, in the shape of AHP |
| `frame` | `frame`, `host` | postMessage between the frame of a view and the host page: MCP Apps of SEP-1865 with `x-air-app` |
| `ide` | `ide`, `air-ide` | The connection of `air-ide`: the frames of air-apps in the vocabulary `x-air-ide` |

## A line

- `{"comment": "..."}` explains the lines after it. The first comment of a file names its setup.
- `{"link", "from", "message"}` is one message. `conn` names the WebSocket when more than one is open: the order of
  its handshake, from 1. Without `conn`, the line is on the WebSocket that opened last.
- `{"link", "from", "raw"}` is a line or a text frame that is no JSON message. `${16MiB+1}` stands for a text of that
  size.
- `{"step": "<kind>", ...}` is an event of the world: `file`, `fileRemoved`, `appStart`, `appExit`, `wsHandshake`,
  `wsClose`, `jcefReload` and `clock`. The first comment of a file defines the steps that it uses.
- `{"state": {"link", "channel", "state"}}` is the mirror of a channel after the lines before it, by the table rule.

## A runner

- A runner plays one or more ends. It sends each line of the other ends, checks each line of its own ends in order,
  and makes each step happen or expects it. It skips the lines of a link that its ends do not touch.
- The order of the lines holds within one link. Across links, a line comes after each line before it that caused it.
- `${any}` matches any value. `${<name>}`, such as `${ticket}`, binds a value at its first place and matches it after.
  `${home}`, `${dir}`, `${project}` and `${port}` are given by the runner. Only the end under test writes `${any}`.
- The files with the fixture app (`registry`, `consent-and-start`, `page-connection`, `view-mirror`, `calls`,
  `requests-to-ide`, `failures`) test `air-ide`: a runner plays the IDE, the host page and a scripted app.
- `frame-bridge.ndjson` tests the host page: a runner plays the frame and `air-ide`.
- An app pins its own side of the app link in a corpus file of its repository, such as
  `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` of Branch Deck. Its lines of `from: app` may hold
  `${any}`, and a runner of `air-ide` does not run it.
