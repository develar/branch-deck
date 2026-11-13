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
| `page` | `page`, `air-ide` | The page entry of `air-ide` on `127.0.0.1:<port>`: `GET /` is the host document, and the WebSocket on the same port carries one JSON-RPC 2.0 object per text frame, in the shape of AHP |
| `frame` | `frame`, `host` | postMessage between the frame of a view and the host page: MCP Apps of SEP-1865 with `x-air-app` |
| `ide` | `ide`, `air-ide` | The connection of `air-ide`: the frames of air-apps in the vocabulary `x-air-ide`, such as the apps channel with its command `appResourceRead`, and the root command `appViewTicket` |

## A line

- `{"comment": "..."}` explains the lines after it. The first comment of a file names its setup.
- `{"link", "from", "message"}` is one message. `conn` names the connection when its link has more than one open: a
  WebSocket of the link `page` by the order of its handshake, a connection of the link `ide` by the order of its
  `initialize`, from 1. Without `conn`, the line is on the connection of its link that opened last.
- `{"link", "from", "raw"}` is a line or a text frame that is no JSON message. `${16MiB+1}` stands for a text of that
  size. `{"link", "from", "binary"}` is a binary frame of a WebSocket, its bytes in base64.
- `{"step": "<kind>", ...}` is an event of the world: `file`, `fileRemoved`, `appStart`, `appExit`, `httpRequest`,
  `wsHandshake`, `wsClose`, `jcefReload`, `pageLoad`, `pageShow`, `pageTheme`, `pageTicket` and `clock`. The first
  comment of a file defines the steps that it uses. The four page steps are steps of the host page, which a runner of
  `air-ide` skips:
  - `pageLoad {url, location}`: the IDE loads the host document with the ticket in the fragment.
  - `pageShow {frame, theme}`: the IDE calls `window.airAppHost.show({frame, theme})` in the main frame of the loaded
    page, which mounts the frame of the view once. `theme` is the theme of the IDE, `{dark, values}`.
  - `pageTheme {theme}`: the IDE calls `window.airAppHost.setTheme(theme)` after a change of its theme.
  - `pageTicket {ticket}`: the IDE calls `window.airAppHost.connect(ticket)` with a new ticket after a drop.
- `{"state": {"link", "channel", "state"}}` is the mirror of a channel after the lines before it, by the table rule.
- The line schema takes a result and the action of an echo as any value. A check of the corpus validates them by the def
  of their method: the answer of `x-air-ide/appViewTicket` by `AppViewTicketResult`, of `initialize` by
  `PageInitializeResult` or `AppInitializeResult`, an action of the apps channel by `AppsAction` or `AppsIntent`, and a
  snapshot of it by `AppsState`.

## A runner

- A runner plays one or more ends. It sends each line of the other ends, checks each line of its own ends in order,
  and makes each step happen or expects it. It skips the lines of a link that its ends do not touch.
- The order of the lines holds within one link. Across links, a line comes after each line before it that caused it.
- `${any}` matches any value. `${<name>}`, such as `${ticket}`, binds a value at its first place and matches it after.
  `${home}`, `${dir}`, `${project}` and `${port}` are given by the runner. Only the end under test writes `${any}`.
- The files with the fixture app (`registry`, `consent-and-start`, `page-connection`, `view-mirror`, `calls`,
  `requests-to-ide`, `failures`) test `air-ide`: a runner plays the IDE, the host page and a scripted app.
- `frame-bridge.ndjson` tests the host page: a runner serves the host document on `http://127.0.0.1:${port}`, plays
  the frame, `air-ide` and the calls of the IDE into the page.
- An app pins its own side of the app link in a corpus file of its repository, such as
  `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` of Branch Deck. Its lines of `from: app` may hold
  `${any}`, and a runner of `air-ide` does not run it.
