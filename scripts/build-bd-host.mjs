// Builds `bd-host` (crates/bd-host), the binary of the remote runs, into src-tauri/binaries for Tauri's `externalBin`.
// Run by the beforeBuildCommand of src-tauri/tauri.macos-bd-host.conf.json, so the bundle carries
// BranchDeck.app/Contents/MacOS/bd-host and `bd install` links ~/.local/bin/bd-host to it. macOS only: a host runs
// the agent under launchd there (docs/design/host-runs.md).
import { buildExternalBin } from "./external-bin.mjs"

buildExternalBin("bd-host", "bd-host")
