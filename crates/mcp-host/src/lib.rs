//! `brd mcp` (`branch-deck cli mcp`): the third host of app-api beside the app and the e2e server, for an IDE with Air
//! (openspec/specs/repository/ide-embedding/spec.md; docs/design/ide-embedding.md). It serves MCP with the vocabulary
//! `x-air-app` over one JSON-RPC connection: the stdin and the stdout of the process for air-ide (`run`), or a
//! WebSocket of the e2e server for the fake host of its tests (`crates/test-server`).
//!
//! - `server`: the connection, its methods, the tools `bd_invoke` and `bd_bootstrap`, the mirrors of the views, the
//!   life of the places and the roots of the project.
//! - `mirror`: a `ViewPatch` of a session as one change of a mirror, by the table rule of Air.
//! - `hosts`: the hosts of app-api in the IDE: the events, the windows as views, and the hosts that do nothing.
//! - `link`: the one queue of the messages brd writes, and its requests to the IDE.
//! - `projects`: the Git roots of the project as the recent projects of the views, the root the main view shows first.
//! - `resources`: the page and the files of the Air build, built into the binary.
//! - `process`: the context of the process on the services of the app and its data directories, and the stdio loop.

pub mod hosts;
pub mod link;
pub mod mirror;
pub mod process;
pub mod projects;
pub mod resources;
pub mod server;
