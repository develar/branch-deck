// Builds a binary of the workspace in release and puts it where Tauri's `externalBin` expects it:
// src-tauri/binaries/<bin>-<target triple>[.exe]. scripts/build-bd.mjs and scripts/build-bd-host.mjs call it from the
// beforeBuildCommand of their Tauri config.
import { execFileSync } from "node:child_process"
import { copyFileSync, mkdirSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")

/** Builds `bin` of the package `pkg` for the target triple of `tauri build` and copies it to src-tauri/binaries. */
export function buildExternalBin(pkg, bin) {
  const host = execFileSync("rustc", ["--print", "host-tuple"], { cwd: root, encoding: "utf8" }).trim()
  // set by `tauri build` for its before-commands
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host
  const exe = triple.includes("windows") ? ".exe" : ""

  // without --target for the host, so the build shares target/release with the app's
  const targetArgs = triple === host ? [] : ["--target", triple]
  execFileSync("cargo", ["build", "--release", "--package", pkg, "--bin", bin, ...targetArgs], { cwd: root, stdio: "inherit" })

  const targetDir = process.env.CARGO_TARGET_DIR || path.join(root, "target")
  const built = path.join(targetDir, ...(triple === host ? [] : [triple]), "release", `${bin}${exe}`)
  const binaries = path.join(root, "src-tauri", "binaries")
  mkdirSync(binaries, { recursive: true })
  copyFileSync(built, path.join(binaries, `${bin}-${triple}${exe}`))
  console.log(`${bin} -> src-tauri/binaries/${bin}-${triple}${exe}`)
}
