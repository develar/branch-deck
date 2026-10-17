// Removes stale cargo units from target/. Cargo never deletes a unit whose hash went out of use (another Cargo.lock,
// profile, clippy/test/build, the hash of a dependency), so a reused treehouse slot piles up dozens of copies of every
// workspace crate within hours. A target dir is not shared between checkouts instead: the unit hash of a path crate
// does not depend on the checkout's path, so two worktrees would overwrite each other's units.
//
//   pnpm prune:target                 dry run for target/ of this checkout
//   pnpm prune:target --pool          also every available treehouse slot of this repository
//   pnpm prune:target --pool --yes    delete
//   pnpm prune:target <checkout>...   the given checkouts instead of this one
//
// In each profile dir (the one with .cargo-lock: debug, release, <triple>/debug, a nested target dir) a unit of
// build/<package>/<hash> or incremental/<crate>-<hash> stays when it is one of the KEEP_NEWEST last built of its
// package or was built in the last KEEP_HOURS hours: a slot builds a dozen variants an hour, so age alone keeps all.
// Deleting a unit that is still in use only costs its rebuild, but a build running in that checkout could fail, so
// --pool takes only slots without a lease and without processes.
import { execFileSync } from "node:child_process"
import { existsSync, lstatSync, readdirSync, rmSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const KEEP_HOURS = 1
const KEEP_NEWEST = 8

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const args = process.argv.slice(2)
const apply = args.includes("--yes")
const pool = args.includes("--pool")
const checkouts = args.filter(arg => !arg.startsWith("--")).map(arg => path.resolve(arg))

const targets = checkouts.length === 0 ? [process.env.CARGO_TARGET_DIR || path.join(root, "target")] : checkouts.map(checkout => path.join(checkout, "target"))
if (pool) {
  const slots = JSON.parse(execFileSync("treehouse", ["status", "--json"], { cwd: root, encoding: "utf8" }))
  for (const slot of slots) {
    if (slot.status === "available" && slot.processes.length === 0) {
      targets.push(path.join(slot.path, "target"))
    }
  }
}

const cutoff = Date.now() - KEEP_HOURS * 60 * 60 * 1000
let total = 0
for (const target of new Set(targets)) {
  if (!existsSync(target)) {
    continue
  }
  let freed = 0
  for (const profileDir of findProfileDirs(target, 4)) {
    freed += pruneUnits(path.join(profileDir, "build"), true)
    freed += pruneUnits(path.join(profileDir, "incremental"), false)
  }
  total += freed
  console.log(`${formatSize(freed).padStart(8)}  ${target}`)
}
console.log(`${apply ? "Freed" : "Would free"} ${formatSize(total)}${apply ? "" : "; run with --yes to delete"}`)

function findProfileDirs(dir, depth) {
  if (existsSync(path.join(dir, ".cargo-lock"))) {
    return [dir]
  }
  if (depth === 0) {
    return []
  }
  return readdirSync(dir, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .flatMap(entry => findProfileDirs(path.join(dir, entry.name), depth - 1))
}

// build/<package>/<hash> is nested; incremental/<crate>-<hash> is flat, and the crate name may itself contain "-"
function pruneUnits(dir, nested) {
  if (!existsSync(dir)) {
    return 0
  }
  const groups = new Map()
  const entries = readdirSync(dir, { withFileTypes: true }).filter(entry => entry.isDirectory())
  for (const entry of entries) {
    if (nested) {
      const packageDir = path.join(dir, entry.name)
      for (const unit of readdirSync(packageDir, { withFileTypes: true })) {
        if (unit.isDirectory()) {
          addUnit(groups, entry.name, path.join(packageDir, unit.name))
        }
      }
    } else {
      addUnit(groups, entry.name.slice(0, entry.name.lastIndexOf("-")), path.join(dir, entry.name))
    }
  }

  let freed = 0
  for (const units of groups.values()) {
    units.sort((a, b) => b.mtime - a.mtime)
    for (const unit of units.slice(KEEP_NEWEST)) {
      if (unit.mtime < cutoff) {
        freed += diskSize(unit.path)
        if (apply) {
          rmSync(unit.path, { recursive: true, force: true })
        }
      }
    }
  }
  return freed
}

function addUnit(groups, name, unitPath) {
  let units = groups.get(name)
  if (units == null) {
    units = []
    groups.set(name, units)
  }
  units.push({ path: unitPath, mtime: lastBuilt(unitPath) })
}

// A rebuild of a unit rewrites its fingerprint files (build/) or adds a session (incremental/), but not always
// the directory entry itself, so the newest mtime among the directory and its direct children counts.
function lastBuilt(unitPath) {
  let mtime = lstatSync(unitPath).mtimeMs
  const fingerprint = path.join(unitPath, "fingerprint")
  const dir = existsSync(fingerprint) ? fingerprint : unitPath
  for (const name of readdirSync(dir)) {
    mtime = Math.max(mtime, lstatSync(path.join(dir, name)).mtimeMs)
  }
  return mtime
}

function diskSize(file) {
  const stat = lstatSync(file)
  if (!stat.isDirectory()) {
    return stat.blocks * 512
  }
  let size = stat.blocks * 512
  for (const name of readdirSync(file)) {
    size += diskSize(path.join(file, name))
  }
  return size
}

function formatSize(bytes) {
  return bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(1)} GB` : `${Math.round(bytes / 1024 ** 2)} MB`
}
