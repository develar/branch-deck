// The builds of the Air page (`vite build --mode air`, docs/design/ide-embedding.md), which the build script of
// crates/mcp-host copies into `brd mcp` (`crates/mcp-host/build/air_build.rs`). Each build goes into a directory of
// its own under `target/dist-air.builds/`, written once; `dist-air` is a link to the newest, replaced by one rename of a
// new link (on Windows a junction, removed first), so a cargo build beside a build of the page never reads half a
// build; every publish touches `crates/mcp-host/air-build.stamp`, on which that build script reruns.
import { existsSync, lstatSync, readdirSync, renameSync, rmSync, symlinkSync, utimesSync } from "node:fs"
import { join, relative } from "node:path"

/** The directory of the builds, below the root of the repository */
export const AIR_BUILDS = join("target", "dist-air.builds")
/** The link a publish creates beside the builds and then renames into the place of `dist-air`: `link-<pid>-<time>` */
const LINK_PREFIX = "link-"
/**
 * How old a link of a publish may be: one older is of a publish that stopped between its two steps, which the next
 * publish removes, and which the build script of mcp-host waits for no more (`STALE_PUBLISH_LINK` of
 * `crates/mcp-host/build/air_build.rs`, the same)
 */
const STALE_LINK_MS = 60_000
/** A directory `dist-air` of a build in place, moved aside by the first publish */
const LEGACY_PREFIX = "legacy-"
/** The stamp the build script of mcp-host reruns on, below the root of the repository */
const STAMP = join("crates", "mcp-host", "air-build.stamp")

/** The directory a new build of the page goes into: named by its time and process, so the names sort as they came */
export function nextAirBuild(root) {
  return join(root, AIR_BUILDS, `${String(Date.now()).padStart(15, "0")}-${process.pid}`)
}

/**
 * Makes `build`, a finished build, the one `dist-air` names, and removes the builds before the last `keep` of those
 * older than it. A directory `dist-air` of a build in place moves aside first
 */
export function publishAirBuild(root, build, { keep = 2 } = {}) {
  if (!existsSync(join(build, "index.html"))) {
    throw new Error(`The Air build ${build} has no index.html: it is not published`)
  }
  const builds = join(root, AIR_BUILDS)
  const dist = join(root, "dist-air")
  removeStaleLinks(builds)
  const link = join(builds, `${LINK_PREFIX}${process.pid}-${Date.now()}`)
  const windows = process.platform === "win32"
  // the link is read where it ends up, at the root: its target relative to the root, so a clone of the checkout keeps
  // its own builds; a junction takes an absolute one
  symlinkSync(windows ? build : relative(root, build), link, windows ? "junction" : "dir")
  const current = lstatSync(dist, { throwIfNoEntry: false })
  if (current?.isDirectory()) {
    renameSync(dist, join(builds, `${LEGACY_PREFIX}${Date.now()}`))
  } else if (current !== undefined && windows) {
    // a rename of a junction onto another one fails on Windows: the old one goes first
    rmSync(dist)
  }
  renameSync(link, dist)
  const now = new Date()
  utimesSync(join(root, STAMP), now, now)
  pruneAirBuilds(builds, build, keep)
}

/** The links of the publishes that stopped between their two steps: older than `STALE_LINK_MS` by the time in their name */
function removeStaleLinks(builds) {
  for (const entry of readdirSync(builds)) {
    const time = entry.startsWith(LINK_PREFIX) ? Number(entry.slice(entry.lastIndexOf("-") + 1)) : Number.NaN
    if (entry.startsWith(LINK_PREFIX) && !(Date.now() - time <= STALE_LINK_MS)) {
      rmSync(join(builds, entry), { force: true })
    }
  }
}

/** The builds older than `published` but the last `keep` of them, and the moved builds in place; nothing newer */
function pruneAirBuilds(builds, published, keep) {
  const name = relative(builds, published)
  const entries = readdirSync(builds).filter(entry => !entry.startsWith(LINK_PREFIX))
  const kept = new Set(
    entries
      .filter(entry => !entry.startsWith(LEGACY_PREFIX) && entry < name)
      .sort()
      .slice(-keep),
  )
  for (const entry of entries) {
    if (entry !== name && (entry.startsWith(LEGACY_PREFIX) || (entry < name && !kept.has(entry)))) {
      rmSync(join(builds, entry), { recursive: true, force: true })
    }
  }
}
