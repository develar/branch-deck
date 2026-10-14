import type { TransferProgress } from "@/lib/bindings"

/** The label of a running fetch or push, e.g. "Pushing 45%"; "Pushing…" until git reports a stage */
export function transferLabel(verb: string, progress: TransferProgress | null): string {
  return progress === null ? `${verb}…` : `${verb} ${progress.percent}%`
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
