import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, statSync, symlinkSync, utimesSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { AIR_BUILDS, nextAirBuild, publishAirBuild } from "./air-dist.mjs"

/** A checkout with the stamp of ide-host, its time an hour ago */
function checkout() {
  const root = mkdtempSync(join(tmpdir(), "air-dist-"))
  mkdirSync(join(root, "crates", "ide-host"), { recursive: true })
  const stamp = join(root, "crates", "ide-host", "air-build.stamp")
  writeFileSync(stamp, "")
  const hourAgo = new Date(Date.now() - 3_600_000)
  utimesSync(stamp, hourAgo, hourAgo)
  return { root, stamp }
}

/** A finished build named `name` below the builds of `root`, whose index.html says it */
function build(root, name) {
  const dir = join(root, AIR_BUILDS, name)
  mkdirSync(join(dir, "assets"), { recursive: true })
  writeFileSync(join(dir, "index.html"), name)
  return dir
}

test("a publish makes dist-air the link to the build and touches the stamp the build script of ide-host reruns on", () => {
  const { root, stamp } = checkout()
  const first = build(root, "000000000000001-1")
  const before = statSync(stamp).mtimeMs
  publishAirBuild(root, first)
  assert.equal(realpathSync(join(root, "dist-air")), realpathSync(first))
  assert.ok(statSync(stamp).mtimeMs > before)
  const second = build(root, "000000000000002-1")
  publishAirBuild(root, second)
  assert.equal(readFileSync(join(root, "dist-air", "index.html"), "utf8"), "000000000000002-1")
  // no link is left beside the builds
  assert.deepEqual(readdirSync(join(root, AIR_BUILDS)).sort(), ["000000000000001-1", "000000000000002-1"])
})

test("a publish keeps the last two builds before it and every newer one, which may still be written", () => {
  const { root } = checkout()
  for (const name of ["000000000000001-1", "000000000000002-1", "000000000000003-1", "000000000000005-1"]) {
    build(root, name)
  }
  publishAirBuild(root, build(root, "000000000000004-1"))
  assert.deepEqual(readdirSync(join(root, AIR_BUILDS)).sort(), ["000000000000002-1", "000000000000003-1", "000000000000004-1", "000000000000005-1"])
})

test("the first publish moves a directory dist-air of a build in place aside and removes it", () => {
  const { root } = checkout()
  mkdirSync(join(root, "dist-air"))
  writeFileSync(join(root, "dist-air", "index.html"), "in place")
  publishAirBuild(root, build(root, "000000000000001-1"))
  assert.equal(readFileSync(join(root, "dist-air", "index.html"), "utf8"), "000000000000001-1")
  assert.deepEqual(readdirSync(join(root, AIR_BUILDS)), ["000000000000001-1"])
})

test("a publish removes the links of publishes that stopped between their steps a minute ago, not a newer one", () => {
  const { root } = checkout()
  const builds = join(root, AIR_BUILDS)
  const first = build(root, "000000000000001-1")
  const stale = join(builds, `link-4242-${Date.now() - 120_000}`)
  const fresh = join(builds, `link-4343-${Date.now()}`)
  symlinkSync(first, stale)
  symlinkSync(first, fresh)
  publishAirBuild(root, build(root, "000000000000002-1"))
  assert.deepEqual(readdirSync(builds).sort(), ["000000000000001-1", "000000000000002-1", `link-4343-${fresh.slice(fresh.lastIndexOf("-") + 1)}`])
})

test("a build without its index.html is not published", () => {
  const { root } = checkout()
  const unfinished = join(root, AIR_BUILDS, "000000000000001-1")
  mkdirSync(unfinished, { recursive: true })
  assert.throws(() => publishAirBuild(root, unfinished), /has no index.html/)
})

test("the builds of a checkout sort as they came", () => {
  const { root } = checkout()
  assert.ok(nextAirBuild(root).startsWith(join(root, AIR_BUILDS)))
  assert.match(nextAirBuild(root), /\d{15}-\d+$/)
})
