import assert from "node:assert/strict"
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { releaseCheckoutLock, retryWhileShared, takeCheckoutLock } from "./checkout-lock.mjs"

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

test("two starts that take over the same dead lock at once: the second keeps it, the first refuses", () => {
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

test("a start holding the takeover of a dead lock makes the others refuse, and it keeps the lock", () => {
  const { dir, lock, isAlive } = setup()
  writeFileSync(lock, "999")
  let refused = 0
  takeCheckoutLock(lock, 1001, {
    isAlive,
    inTakeover: () => {
      assert.throws(() => takeCheckoutLock(lock, 1002, { isAlive }), /Another pnpm e2e runs in this checkout \(pid 1001\)/)
      refused++
    },
  })
  assert.equal(refused, 1)
  assert.equal(readFileSync(lock, "utf8"), "1001")
  assert.deepEqual(readdirSync(dir), ["e2e-run.lock"])
})

/**
 * Runs the start `pid` against `lock`, and each start of `inner` whole at the step of `pid` it is keyed by (an
 * `afterRead`, an `inTakeover`, a check of a pid), its own `inner` the same way; sets the outcome of each by its pid
 */
function runInterleaved(lock, running, [pid, inner], outcomes) {
  let step = 0
  const atStep = () => {
    const nested = inner.get(step++)
    if (nested !== undefined) {
      runInterleaved(lock, running, nested, outcomes)
    }
  }
  const isAlive = other => {
    atStep()
    return running.has(other)
  }
  try {
    takeCheckoutLock(lock, pid, { isAlive, afterRead: atStep, inTakeover: atStep })
    outcomes.set(pid, "owner")
  } catch (error) {
    outcomes.set(pid, error.message)
  }
}

test("three starts that take over the same dead lock at once, in every order of their steps: one owns it, the others refuse, nothing else is left", () => {
  const running = new Set([1001, 1002, 1003])
  const steps = Array.from({ length: 8 }, (_, step) => step)
  // the second start whole at a step of the first, the third at a later step of the first or at a step of the second
  const schedules = steps.flatMap(i =>
    steps.flatMap(j => [
      [1001, new Map([[i, [1002, new Map([[j, [1003, new Map()]]])]]])],
      ...(j > i
        ? [
            [
              1001,
              new Map([
                [i, [1002, new Map()]],
                [j, [1003, new Map()]],
              ]),
            ],
          ]
        : []),
    ]),
  )
  // a lock whose run ended; also with the takeover of a start that ended within it
  const lefts = [{ "e2e-run.lock": "999" }, { "e2e-run.lock": "999", "e2e-run.lock.takeover": "998" }]
  const dir = mkdtempSync(join(tmpdir(), "checkout-lock-"))
  const lock = join(dir, "e2e-run.lock")
  let ran = 0
  for (const left of lefts) {
    for (const schedule of schedules) {
      for (const name of readdirSync(dir)) {
        rmSync(join(dir, name))
      }
      for (const [name, pid] of Object.entries(left)) {
        writeFileSync(join(dir, name), pid)
      }
      const outcomes = new Map()
      runInterleaved(lock, running, schedule, outcomes)
      if (outcomes.size === 3) {
        ran++
        const case_ = `${JSON.stringify(left)} ${JSON.stringify(schedule, (_, value) => (value instanceof Map ? [...value] : value))}`
        const owners = [...outcomes].filter(([, outcome]) => outcome === "owner").map(([pid]) => pid)
        assert.equal(owners.length, 1, `one owner: ${case_} ${JSON.stringify([...outcomes])}`)
        assert.equal(readFileSync(lock, "utf8"), String(owners[0]), case_)
        for (const [, outcome] of outcomes) {
          assert.match(outcome, /^owner$|^Another pnpm e2e runs in this checkout \(pid 100[123]\)/, case_)
        }
        assert.deepEqual(readdirSync(dir), ["e2e-run.lock"], case_)
      }
    }
  }
  assert.ok(ran > 50, `${ran} orders ran all three starts`)
})

test("a start removes the files of the starts that ended, the pid in their names, and keeps those of running ones", () => {
  const { dir, lock, isAlive } = setup()
  for (const name of ["e2e-run.lock.999", "e2e-run.lock.stale.999", "e2e-run.lock.takeover.stale.998", "e2e-run.lock.1002", "notes.txt"]) {
    writeFileSync(join(dir, name), name.match(/\d+/)?.[0] ?? "")
  }
  takeCheckoutLock(lock, 1001, { isAlive })
  assert.deepEqual(readdirSync(dir).sort(), ["e2e-run.lock", "e2e-run.lock.1002", "notes.txt"])
})

test("on Windows a call another process holds is tried again for about a second; elsewhere, and for any other error, once", () => {
  /** A call that fails with `code` its first `times` calls, then answers the count of its calls */
  const failing = (code, times) => {
    let calls = 0
    return () => {
      calls++
      if (calls <= times) {
        throw Object.assign(new Error(code), { code })
      }
      return calls
    }
  }
  const waits = []
  const sleep = ms => waits.push(ms)
  assert.equal(retryWhileShared(failing("EBUSY", 2), { windows: true, sleep }), 3)
  assert.deepEqual(waits, [1, 5])
  waits.length = 0
  assert.throws(() => retryWhileShared(failing("EPERM", Infinity), { windows: true, sleep }), { code: "EPERM" })
  assert.equal(
    waits.reduce((sum, ms) => sum + ms, 0),
    941,
  )
  waits.length = 0
  assert.throws(() => retryWhileShared(failing("ENOENT", 1), { windows: true, sleep }), { code: "ENOENT" })
  assert.throws(() => retryWhileShared(failing("EPERM", 1), { windows: false, sleep }), { code: "EPERM" })
  assert.deepEqual(waits, [])
})
