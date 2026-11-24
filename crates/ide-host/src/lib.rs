//! The bridge `branch-deck ide` (`brd mcp`, `branch-deck cli mcp`) of an IDE with Air
//! (openspec/specs/repository/ide-embedding/spec.md; docs/design/ide-embedding.md): MCP with the vocabulary `x-air-app`
//! over one JSON-RPC connection with air-ide (the stdin and the stdout of the process, `run`, or a WebSocket of the e2e
//! server for the fake host of its tests, `crates/test-server`) on one side, a client of the one backend of the user
//! (docs/design/backend-protocol.md) on the other: the desktop app, which the bridge starts without a window when it
//! does not run, or a backend in its own process.
//!
//! - `server`: the connection with air-ide, its methods, the tools `bd_invoke` and `bd_bootstrap`, the mirrors of the
//!   views over the repository channels, the life of the places, the host requests and the roots of the project.
//! - `backend`: the connection to the backend: its `initialize`, its requests and their answers, its transports.
//! - `start`: how the bridge reaches the backend: it joins it, starts the app, or serves alone.
//! - `fallback`: the backend of the bridge's process, and the way back to the app's.
//! - `mirror`: a `ViewPatch` of a session as one change of a mirror, by the table rule of Air.
//! - `hosts`: the windows of the bridge as the views of the IDE.
//! - `link`: the one queue of the messages the bridge writes to the IDE, and its requests to the IDE.
//! - `notices`: the failure of a robot as a notification of the IDE, whose action reveals its branch, claimed once.
//! - `projects`: the Git roots of the project as the recent projects of the views, the root the main view shows first.
//! - `settings`: the settings as the backend told the bridge.
//! - `resources`: the page and the files of the Air build, built into the binary.
//! - `process`: the stdio loop, the choice of the backend and the exit code.

// the module of the build script that reads the Air build of the page, here for its test
#[cfg(test)]
#[path = "../build/air_build.rs"]
mod air_build;
#[cfg(test)]
#[path = "air_build_test.rs"]
mod air_build_test;
pub mod backend;
pub mod fallback;
pub mod hosts;
pub mod link;
pub mod mirror;
pub mod notices;
pub mod process;
pub mod projects;
pub mod resources;
pub mod server;
pub mod settings;
pub mod start;
