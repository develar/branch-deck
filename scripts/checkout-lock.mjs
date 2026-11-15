// The lock of a checkout for one run of `pnpm e2e` (`target/e2e-run.lock`, the global setup of the e2e tests): a second
// run in the same checkout, also on another port, refuses at once, for its global setup would build `dist/` anew under
// the pages of the first. The lock is a hard link of a file the pid is written in first, so no start reads it empty.
//
// A lock whose run has ended (its process gone) is taken over, one start at a time: the start holds the takeover
// (`<lock>.takeover`, made the same way, exclusive as an O_EXCL create is and never empty), reads the lock again and
// removes it only if its run is still gone, then links its own. Only the holder of the takeover removes a lock, so what
// it read stays until it removes it; a start that finds the takeover held refuses, as the holder runs or refuses for a
// run that does. A takeover whose start ended within it (a few calls long) is moved aside and dropped only if it is
// still that start's, so the window of a third start remains only after such an end.
//
// A start removes the files of the starts that ended before them (`<lock>.<pid>`, `<lock>.stale.<pid>`,
// `<lock>.takeover.stale.<pid>`, the pid in the name), and on Windows tries a removal, a rename and a link again for
// about a second while another process holds the file (an antivirus, an indexer), as `sync_utils::fs_retry` does.
import { linkSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { basename, dirname, join } from "node:path"

/** The waits before the tries after the first of a call Windows refuses for a holder: about a second in all */
const RETRY_DELAYS_MS = [1, 5, 10, 25, 50, 100, 250, 500]

function sleepSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}

/**
 * `call`, tried again while Windows refuses it for another process that holds the file (`EBUSY`, `EPERM`: a program
 * that opened it without sharing delete), about a second (`sync_utils::fs_retry`); elsewhere `call` once
 */
export function retryWhileShared(call, { windows = process.platform === "win32", sleep = sleepSync } = {}) {
  const delays = RETRY_DELAYS_MS[Symbol.iterator]()
  for (;;) {
    try {
      return call()
    } catch (error) {
      const delay = windows && (error.code === "EBUSY" || error.code === "EPERM") ? delays.next() : { done: true }
      if (delay.done) {
        throw error
      }
      sleep(delay.value)
    }
  }
}

/** Whether `pid` runs: a process of another user answers EPERM, which runs too */
function processRuns(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    return error.code === "EPERM"
  }
}

/** The pid a lock file holds; null for none, or one that is no file */
function pidOf(file) {
  try {
    const pid = Number(readFileSync(file, "utf8"))
    return Number.isInteger(pid) && pid > 0 ? pid : null
  } catch {
    return null
  }
}

/** Links `existing` as `link`; false when `link` exists */
function linkIfFree(existing, link) {
  try {
    retryWhileShared(() => linkSync(existing, link))
    return true
  } catch (error) {
    if (error.code === "EEXIST") {
      return false
    }
    throw error
  }
}

function remove(file) {
  retryWhileShared(() => rmSync(file, { force: true }))
}

function refusal(holder) {
  return new Error(
    `Another pnpm e2e runs in this checkout (pid ${holder}): one at a time, as its global setup builds dist/ under the other. Wait for it, or run in another checkout`,
  )
}

/** Removes the files beside `lock` of the starts that ended (the pid in their names), not those of `pid` */
function removeEnded(lock, pid, isAlive) {
  const prefix = `${basename(lock)}.`
  const dir = dirname(lock)
  for (const name of readdirSync(dir)) {
    const match = name.startsWith(prefix) ? /^(?:stale\.|takeover\.stale\.)?(\d+)$/.exec(name.slice(prefix.length)) : null
    const owner = match === null ? null : Number(match[1])
    if (owner !== null && owner !== pid && !isAlive(owner)) {
      remove(join(dir, name))
    }
  }
}

/**
 * Moves aside the takeover `takeover` its start `taker` left as it ended (the module docs) and drops it if it is still
 * that one's; one a start took after the read goes back in its place, and this start refuses
 */
function dropEndedTakeover(takeover, taker, pid, isAlive) {
  const aside = `${takeover}.stale.${pid}`
  try {
    retryWhileShared(() => renameSync(takeover, aside))
  } catch (error) {
    if (error.code === "ENOENT") {
      return
    }
    throw error
  }
  const moved = pidOf(aside)
  if (moved === taker || moved === null || moved === pid || !isAlive(moved)) {
    remove(aside)
    return
  }
  linkIfFree(aside, takeover)
  remove(aside)
  throw refusal(moved)
}

/**
 * Takes over the lock `lock` of a run that ended, holding the takeover (the module docs): true once the lock is the one
 * of `mine`, false when it is to be read again
 *
 * @throws the refusal of a takeover another start holds
 */
function takeOver(lock, mine, pid, isAlive, inTakeover) {
  const takeover = `${lock}.takeover`
  if (!linkIfFree(mine, takeover)) {
    const taker = pidOf(takeover)
    if (taker !== null && taker !== pid && isAlive(taker)) {
      throw refusal(taker)
    }
    dropEndedTakeover(takeover, taker, pid, isAlive)
    return false
  }
  try {
    inTakeover()
    const holder = pidOf(lock)
    if (holder !== null && holder !== pid && isAlive(holder)) {
      return false
    }
    remove(lock)
    return linkIfFree(mine, lock)
  } finally {
    if (pidOf(takeover) === pid) {
      remove(takeover)
    }
  }
}

/**
 * Takes the lock `lock` for the run `pid` (the module docs). `isAlive` tells whether a pid runs; `afterRead` runs between
 * the read of a holder that ended and the takeover, `inTakeover` while this start holds it: there a test lets other
 * starts run
 *
 * @throws the refusal of a lock another run holds
 */
export function takeCheckoutLock(lock, pid, { isAlive = processRuns, afterRead = () => {}, inTakeover = () => {} } = {}) {
  removeEnded(lock, pid, isAlive)
  const mine = `${lock}.${pid}`
  writeFileSync(mine, String(pid))
  try {
    for (;;) {
      if (linkIfFree(mine, lock)) {
        return
      }
      const holder = pidOf(lock)
      if (holder !== null && holder !== pid && isAlive(holder)) {
        throw refusal(holder)
      }
      afterRead()
      if (takeOver(lock, mine, pid, isAlive, inTakeover)) {
        return
      }
    }
  } finally {
    remove(mine)
  }
}

/** Drops the lock `lock` of the run `pid`; a lock of another run stays */
export function releaseCheckoutLock(lock, pid) {
  if (pidOf(lock) === pid) {
    remove(lock)
  }
}
