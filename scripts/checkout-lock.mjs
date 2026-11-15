// The lock of a checkout for one run of `pnpm e2e` (`target/e2e-run.lock`, the global setup of the e2e tests): a second
// run in the same checkout, also on another port, refuses at once, for its global setup would build `dist/` anew under
// the pages of the first. The lock is a hard link of a file the pid is written in first, so no start reads it empty; a
// lock whose run has ended (its process gone) is taken over by moving it aside under a name of this run first and
// dropping it only if it is still the lock of that dead run, so two starts that take over the same dead lock never both
// run (one moves aside the lock the other just took, and puts it back).
import { linkSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"

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

function refusal(holder) {
  return new Error(
    `Another pnpm e2e runs in this checkout (pid ${holder}): one at a time, as its global setup builds dist/ under the other. Wait for it, or run in another checkout`,
  )
}

/**
 * Takes the lock `lock` for the run `pid` (the module docs). `isAlive` tells whether a pid runs; `afterRead` runs between
 * the read of a dead holder and the move of its lock, where a test lets another start take the lock
 *
 * @throws the refusal of a lock another run holds
 */
export function takeCheckoutLock(lock, pid, { isAlive = processRuns, afterRead = () => {} } = {}) {
  const mine = `${lock}.${pid}`
  writeFileSync(mine, String(pid))
  try {
    for (;;) {
      try {
        linkSync(mine, lock)
        return
      } catch (error) {
        if (error.code !== "EEXIST") {
          throw error
        }
      }
      const holder = pidOf(lock)
      if (holder !== null && holder !== pid && isAlive(holder)) {
        throw refusal(holder)
      }
      afterRead()
      const stale = `${lock}.stale.${pid}`
      try {
        renameSync(lock, stale)
      } catch (error) {
        if (error.code === "ENOENT") {
          continue
        }
        throw error
      }
      const moved = pidOf(stale)
      if (moved === holder || moved === null || moved === pid || !isAlive(moved)) {
        rmSync(stale, { force: true })
        continue
      }
      // the lock of a run that took it after the read: back in its place, and this run refuses
      try {
        linkSync(stale, lock)
      } catch (error) {
        if (error.code !== "EEXIST") {
          throw error
        }
      }
      rmSync(stale, { force: true })
      throw refusal(moved)
    }
  } finally {
    rmSync(mine, { force: true })
  }
}

/** Drops the lock `lock` of the run `pid`; a lock of another run stays */
export function releaseCheckoutLock(lock, pid) {
  if (pidOf(lock) === pid) {
    rmSync(lock, { force: true })
  }
}
