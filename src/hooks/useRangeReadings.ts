import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import type { RangeReadingView, RangeReadingsEvent, ReadingRange, StackReading } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"

/*
 * What the ranges a review window shows leave to read (openspec/specs/review/range-reading/spec.md): `get_range_readings`
 * of the pull request of its header, or of every layer of a stack, read again when a range changes. Rust sends the
 * readings as they stand, again as each range is read and the index answers its renames, each event replacing the last;
 * `disabled` for a repository that turns the review plan off. One read of the window is in flight: Rust supersedes the
 * older one, which tells nothing more.
 */

/** The readings of the ranges asked, by their place, null for one not read yet, and their sum */
export interface RangeReadings {
  readonly readings: readonly (RangeReadingView | null)[]
  readonly stack: StackReading
}

/** The readings of `ranges` in `repositoryPath`; null while none is known, with the plan turned off, and for no ranges */
export function useRangeReadings(repositoryPath: string, ranges: readonly ReadingRange[] | null): RangeReadings | null {
  const key = ranges === null || ranges.length === 0 ? null : JSON.stringify([repositoryPath, ranges])
  const [answer, setAnswer] = useState<{ readonly key: string; readonly readings: RangeReadings | null } | null>(null)
  useEffect(() => {
    if (key === null) {
      return
    }
    const [path, asked] = JSON.parse(key) as [string, ReadingRange[]]
    let current = true
    const events = new Channel<RangeReadingsEvent>()
    events.onmessage = event => {
      if (current) {
        setAnswer({ key, readings: event.kind === "disabled" ? null : { readings: event.readings, stack: event.stack } })
      }
    }
    void commands.getRangeReadings({ repositoryPath: path, ranges: asked }, events).then(result => {
      if (current && result.status === "error" && result.error.code !== "canceled") {
        logError("Cannot read what the changes leave to read", result.error)
      }
    })
    return () => {
      current = false
    }
  }, [key])
  return answer !== null && answer.key === key ? answer.readings : null
}
