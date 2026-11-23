## ADDED Requirements

### Requirement: Wire: Every message is one JSON-RPC 2.0 object on one line

A connection MUST carry one JSON-RPC 2.0 object per line, of 16 MiB at most, and every `params` MUST name its
`channel`: `ahp-root://` for a connection command and a root command, `x-branch-deck:/repository/<n>` for a channel
command. A method keeps its AHP name where its meaning is AHP's, and is `x-branch-deck/<name>` else.

#### Scenario: A batch array
- **WHEN** a client sends a JSON array of two requests on one line
- **THEN** the host answers `-32600` with `data.reason` `invalidRequest` and the id `null`, and serves neither

#### Scenario: A method without a channel
- **WHEN** a client sends `ping` without `channel`
- **THEN** the host answers `-32602 badParams`

### Requirement: Wire: Initialize names the versions, the builds and the capabilities

The first request of a connection MUST be `initialize` with the offered AHP versions, `clientId`, `clientInfo.build`
and `_meta["x-branch-deck"]` with the vocabulary version and the capabilities of the client. The host SHALL answer the
highest version it serves in their caret ranges, `serverSeq`, `serverInfo.build` and its vocabulary version.

#### Scenario: An accepted offer
- **WHEN** a client offers `1.0.0` with the build of the host
- **THEN** the answer has `protocolVersion` `1.0.0`, `serverSeq`, `snapshots: []` and `serverInfo` with `name`
  `branch-deck` and the build

#### Scenario: A request before initialize
- **WHEN** a client sends `subscribe` before `initialize`
- **THEN** the host answers `-32600 notInitialized`, and a second `initialize` after the first is
  `-32600 alreadyInitialized`

#### Scenario: No common version
- **WHEN** a client offers only `2.0.0`
- **THEN** the host answers `-32005 ahpVersion`

### Requirement: Errors: A failure of the protocol is a code with data.reason

A failure of the protocol MUST be a JSON-RPC error whose code is a JSON-RPC or an AHP code and whose `data.reason`
names its kind in camelCase; an app-api refusal of a method without an answer of its own SHALL be `-32603 apiError`
with the `ApiError` in `data.apiError`. A receiver MUST ignore a field of `data` it does not know.

#### Scenario: An unknown method
- **WHEN** a client asks `x-branch-deck/nothing`
- **THEN** the answer is `-32601 unknownMethod` with the id of the request

#### Scenario: A line that is not JSON
- **WHEN** a client sends a line that does not parse
- **THEN** the answer is `-32700 parse` with the id `null`, and the connection stays open

### Requirement: Socket: The socket of a build is owner-only

The socket MUST be `<base>/branch-deck-<key>/backend.sock`, the key 16 hex digits of a hash of the data directory and
the path of the executable, in a directory of its owner with mode 0700, and both sides MUST refuse a directory that is
not; on Windows the pipe `\\.\pipe\branch-deck-<key>` with a DACL of its owner and remote clients refused.

#### Scenario: A dev build beside a release
- **WHEN** a dev build and a release of the app run as one user
- **THEN** each has its own socket and its own backend

#### Scenario: A directory of another mode
- **WHEN** the directory of the key exists with mode 0755
- **THEN** the backend opens no socket and logs why, and the bridge does not connect to it

### Requirement: Socket: A peer of another user is refused

The host MUST close a connection whose peer runs as another user (its uid on Unix, its SID on Windows), and a client
MUST close a connection to a server that runs as another user, before either reads a message.

#### Scenario: The uid of the peer
- **WHEN** a process of another uid connects to the socket
- **THEN** the host closes the connection without an answer

### Requirement: Socket: One backend per key holds backend.lock

A backend MUST hold `backend.lock` in the directory of its key for the life of its process and release it at its exit;
a starting backend SHALL retry the lock for a few seconds, and its holder removes a stale socket before it listens.
A second start of the app MUST connect and send `x-branch-deck/showMain`, which shows the main window.

#### Scenario: A second start of the app
- **WHEN** the app starts while its backend runs without a window
- **THEN** the backend builds and shows the main window, and the second process exits

#### Scenario: A stale socket
- **WHEN** a backend starts and finds a socket file that no process listens on
- **THEN** it removes the file under the lock and listens

### Requirement: Socket: The stale side yields to a client of another build

An `initialize` whose `clientInfo.build` is not the host's MUST be `-32005 otherBuild` with the host's build and
`exits`. The side whose executable on disk is no longer its build yields: a stale backend without a window exits after
the answer, a stale backend with a window stays and offers Restart to Update, and a stale bridge exits with 1.

#### Scenario: The app updated in place, no window
- **WHEN** a bridge of the new build connects to a backend of the old one that has no window
- **THEN** the answer is `otherBuild` with `exits: true`, the backend exits, and the bridge starts the app of its build

#### Scenario: The app updated in place, a window open
- **WHEN** a bridge of the new build connects to a backend of the old one that shows a window
- **THEN** the answer is `otherBuild` with `exits: false`, the backend tells Restart to Update, and the bridge runs its
  backend in its own process

### Requirement: Events: The events of app-api go to every connection

An event of app-api MUST be `x-branch-deck/event {event, payload, window?}` to every connection, without `window` to
each of its windows, and one to the main window to the main window of each connection; an event a page emits SHALL stay
inside its connection.

#### Scenario: A setting changed in the IDE
- **WHEN** a window of the IDE sets a setting
- **THEN** every window of the app gets `settings_changed` with it

#### Scenario: An event of a page
- **WHEN** a review window of the IDE emits `reveal-branch` to its main window
- **THEN** the main view of the IDE gets it, and no window of the app does

### Requirement: Events: A news is told once by the backend

A client MUST claim a news before it tells it (`x-branch-deck/claimNews {repositoryPath, news}`), and the host SHALL
answer `first: true` to the first claim of that news of that repository and `false` to every later one. The desktop
page claims nothing until it goes through the host, so until then the app and the IDE may both tell a news.

#### Scenario: The app and the IDE on one repository
- **WHEN** a robot of a repository fails while the app and the IDE show it, neither with the focus
- **THEN** one of them tells the failure, and the other does not
