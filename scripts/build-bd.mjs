// Builds the console `bd` (crates/bd-cli) and puts it where Tauri's `externalBin` expects it:
// src-tauri/binaries/bd-<target triple>[.exe]. Run by the beforeBuildCommand of src-tauri/tauri.windows-bd.conf.json, so
// the installer puts bd.exe next to the app. Windows only: the app binary there has no console.
import { execFileSync } from "node:child_process"
import { copyFileSync, mkdirSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const host = execFileSync("rustc", ["--print", "host-tuple"], { cwd: root, encoding: "utf8" }).trim()
// set by `tauri build` for its before-commands
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host
const exe = triple.includes("windows") ? ".exe" : ""

// without --target for the host, so the build shares target/release with the app's
const targetArgs = triple === host ? [] : ["--target", triple]
execFileSync("cargo", ["build", "--release", "--package", "bd-cli", "--bin", "bd", ...targetArgs], { cwd: root, stdio: "inherit" })

const targetDir = process.env.CARGO_TARGET_DIR || path.join(root, "target")
const built = path.join(targetDir, ...(triple === host ? [] : [triple]), "release", `bd${exe}`)
const binaries = path.join(root, "src-tauri", "binaries")
mkdirSync(binaries, { recursive: true })
copyFileSync(built, path.join(binaries, `bd-${triple}${exe}`))
console.log(`bd -> src-tauri/binaries/bd-${triple}${exe}`)
