import assert from "node:assert/strict"
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { releaseCheckoutLock, takeCheckoutLock } from "./checkout-lock.mjs"

/** A lock path in a directory of its own, and the pids that run: 1001 and 1002 (999 ended) */
function setup() {
  const dir = mkdtempSync(join(tmpdir(), "checkout-lock-"))
  const running = new Set([1001, 1002])
  return { dir, lock: join(dir, "e2e-run.lock"), isAlive: pid => running.has(pid), running }
}

test("a free checkout is taken, a run that holds it refuses another, and the release frees it", () => {
  const { dir, lock, isAlive } = setup()
  takeCheckoutLock(lock, 1001, { isAlive })
  assert.equal(readFileSync(lock, "utf8"), "1001")
  assert.throws(() => takeCheckoutLock(lock, 1002, { isAlive }), /Another pnpm e2e runs in this checkout \(pid 1001\)/)
  releaseCheckoutLock(lock, 1002)
  assert.equal(readFileSync(lock, "utf8"), "1001", "a run releases only its own lock")
  releaseCheckoutLock(lock, 1001)
  assert.deepEqual(readdirSync(dir), [])
})

test("the lock of a run that ended is taken over, and nothing of either is left beside it", () => {
  const { dir, lock, isAlive } = setup()
  writeFileSync(lock, "999")
  takeCheckoutLock(lock, 1001, { isAlive })
  assert.equal(readFileSync(lock, "utf8"), "1001")
  assert.deepEqual(readdirSync(dir), ["e2e-run.lock"])
})

test("two starts that take over the same dead lock at once: the second keeps it, the first puts it back and refuses", () => {
  const { dir, lock, isAlive } = setup()
  writeFileSync(lock, "999")
  // the first read the dead holder; before it moves the lock, the second takes it over whole
  const first = () =>
    takeCheckoutLock(lock, 1001, {
      isAlive,
      afterRead: () => takeCheckoutLock(lock, 1002, { isAlive }),
    })
  assert.throws(first, /Another pnpm e2e runs in this checkout \(pid 1002\)/)
  assert.equal(readFileSync(lock, "utf8"), "1002")
  assert.deepEqual(readdirSync(dir), ["e2e-run.lock"])
})
