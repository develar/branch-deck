import type { TransferProgress } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"

/**
 * How far a push is, as the page shows it: what its channel told, or, while it waits for another holder of the
 * repository lock (the step of its operation in the view, `lockWaitProgress`), whom for, with Stop Waiting
 */
export interface PushProgress extends TransferProgress {
  /** It has not started: it waits for another holder of the repository lock, whom `line` names */
  readonly waiting?: boolean
  readonly stopWaiting?: () => void
}

/**
 * A push that waits for another holder of the repository lock: `words` say whom for (`presentLockWait`), `stop` ends
 * the wait with nothing pushed
 */
export function lockWaitProgress(words: string, stop: () => void): PushProgress {
  return { percent: 0, transferred: null, speed: null, line: words, waiting: true, stopWaiting: stop }
}

/**
 * Stop Waiting of a push that waits for another holder of the repository lock, as the hover of its button and the step
 * of its Safe Push offer it; null while it waits for nobody
 */
export function stopWaitingPushItem(progress: PushProgress | null): MenuItem | null {
  const stop = progress?.stopWaiting
  if (stop === undefined) {
    return null
  }
  return { id: "stop-waiting", label: "Stop Waiting", description: `${progress?.line}; stop and push nothing`, onSelect: stop }
}

/**
 * The label of a running fetch or push, e.g. "Pushing 45%"; "Pushing…" until git reports a stage; while the push waits
 * for another holder of the repository lock, who it is: "Waiting for `bd pick` (pid 4242, for 12 s)"
 */
export function transferLabel(verb: string, progress: PushProgress | null): string {
  if (progress?.waiting === true) {
    return progress.line
  }
  return progress === null ? `${verb}…` : `${verb} ${progress.percent}%`
}

/** The percent a compact label shows; null before git reports a stage and while the push waits for the lock */
export function transferPercent(progress: PushProgress | null): number | null {
  return progress === null || progress.waiting === true ? null : progress.percent
}

/** What a fetch received or a push sent so far and how fast, as git prints it, e.g. "5.79 MiB · 32.57 MiB/s"; null before git says */
export function transferAmount(progress: TransferProgress | null): string | null {
  if (progress?.transferred == null) {
    return null
  }
  return progress.speed === null ? progress.transferred : `${progress.transferred} · ${progress.speed}`
}

/**
 * `transferAmount` for a line below git's last progress line: null when that line says the amount already
 * ("Writing objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s"), the amount after a stage end that doesn't
 */
export function transferAmountNotInLine(progress: TransferProgress | null): string | null {
  if (progress?.transferred == null || progress.line.includes(progress.transferred)) {
    return null
  }
  return transferAmount(progress)
}
