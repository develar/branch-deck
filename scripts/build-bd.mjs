// Builds the console `bd` (crates/bd-cli) into src-tauri/binaries for Tauri's `externalBin`. Run by the
// beforeBuildCommand of src-tauri/tauri.windows-bd.conf.json, so the installer puts bd.exe next to the app. Windows
// only: the app binary there has no console.
import { buildExternalBin } from "./external-bin.mjs"

buildExternalBin("bd-cli", "bd")
