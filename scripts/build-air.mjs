// `pnpm build:air`: the Air build of the page (`vite build --mode air`, docs/design/ide-embedding.md) into `dist-air/`,
// which the build script of crates/mcp-host copies into `brd mcp`. Vite empties its outDir and writes it file by file,
// so a cargo build beside it (the e2e server of the next run, nextest, clippy) read a half-written build: "couldn't read
// dist-air/assets/…". The page is built into `target/dist-air.next/` and takes the place of `dist-air/` whole: two
// renames, between which the build script finds the next build (`crates/mcp-host/build.rs`)
import { execSync } from "node:child_process"
import { existsSync, renameSync, rmSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const dist = join(root, "dist-air")
const next = join(root, "target", "dist-air.next")
const old = join(root, "target", "dist-air.old")

rmSync(next, { recursive: true, force: true })
// through the shell, which finds `vp.cmd` on Windows too (`beforeBuildCommand` of src-tauri/tauri.windows-bd.conf.json);
// the plugin of the Air build takes its directory (`airBuild` of vite.config.ts)
execSync("vp build --mode air --emptyOutDir", { cwd: root, stdio: "inherit", env: { ...process.env, BRANCH_DECK_AIR_OUT_DIR: next } })
rmSync(old, { recursive: true, force: true })
if (existsSync(dist)) {
  renameSync(dist, old)
}
renameSync(next, dist)
rmSync(old, { recursive: true, force: true })
