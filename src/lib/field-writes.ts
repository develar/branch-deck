import type { WriteOrder } from "@/lib/bindings"

/**
 * The writes of the typing of one field of Settings (`useConfigDraft`): a blur, Enter and the close of the window each
 * save the field, so one change must be one write, and no change may be lost. Whether a save has anything to write is
 * decided here: while no write runs, a value the field shows (`shown`) writes nothing; while a write runs, the save waits
 * for it and then writes its own value, the latest of those saves, unless it is the value that write took or refused (a
 * blur after Enter, a close during the write of a blur write nothing more; an edit typed during the write is written
 * after it, also one back to the value shown before, which `shown` still is until the answer). The close of the window
 * sends its value at once (`now`), beside the write that runs: each write of the page has its number (`nextWriteOrder`),
 * and the one writer of what it writes applies the newer one whichever comes first (openspec/specs/repository/settings/
 * spec.md, "The order of the writes of a field"). A save of the value a write took writes nothing again until the field
 * dropped its typing (`typingCleared`): the page renders the field without it only after the write answered, and a close
 * of the window between that answer and that render saves the typing of the render before (two writes of one change
 * before, repository-settings.spec.ts "should save valid typing when the window closes, and nothing else").
 */
export class FieldWrites {
  private running: Promise<WriteFailure | null> | null = null
  /** The value of the write that runs, while one does */
  private runningValue: string | null = null
  private written: string | null = null
  /** The latest save that came while a write ran: written once that write ended */
  private next: FieldSave | null = null
  /** The writes started, and the latest of them the backend took: an older one that answers later takes nothing */
  private started = 0
  private latestTaken = 0

  /**
   * Writes `value` with `write`, unless the field shows it (`shown`) and no write runs; `taken` runs with the value after
   * a write the backend took and for a value the field shows, so the field drops its typing, only while the typing is
   * that value. With `now` (the close of the window), a value other than the one of the write that runs is written at
   * once instead of after it. Resolves once the writes that serve the save ended, at once when none is needed, with the
   * first failure a write answered, else null; rejects with an error a write threw, after the saves queued behind it,
   * and the field writes again
   */
  save(value: string, shown: string, write: () => Promise<WriteAnswer>, taken: (value: string) => void, now = false): Promise<WriteFailure | null> {
    const save: FieldSave = { value, write, taken }
    if (this.running !== null) {
      if (!now) {
        this.next = save
        return this.running
      }
      this.next = null
      if (value === this.runningValue) {
        return this.running
      }
      const running = this.running
      return this.writeOne(save).then(async failure => (await running) ?? failure)
    }
    if (value === shown) {
      taken(value)
      return Promise.resolve(null)
    }
    if (value === this.written) {
      return Promise.resolve(null)
    }
    const running = this.run(save)
    this.running = running
    return running
  }

  /** The field shows no typing any more: the value it shows is the one written, and a typing anew is written anew */
  typingCleared(): void {
    this.written = null
  }

  /**
   * Writes `first`, then each latest save that came meanwhile with another value, or with any value after a write that
   * threw; resolves with the first failure a write answered, and rejects with the first error a write threw,
   * once the saves queued behind it were written
   */
  private async run(first: FieldSave): Promise<WriteFailure | null> {
    let thrown: { error: unknown } | null = null
    let failure: WriteFailure | null = null
    try {
      let current: FieldSave | null = first
      while (current !== null) {
        const save: FieldSave = current
        this.runningValue = save.value
        let threw = false
        try {
          failure ??= await this.writeOne(save)
        } catch (error: unknown) {
          threw = true
          thrown ??= { error }
        }
        const next: FieldSave | null = this.next
        this.next = null
        current = next !== null && (threw || next.value !== save.value) ? next : null
      }
    } finally {
      this.running = null
      this.runningValue = null
    }
    if (thrown !== null) {
      throw thrown.error
    }
    return failure
  }

  /** One write of `save`: its failure, or null for a value the backend took or a write a newer one of the key superseded */
  private async writeOne(save: FieldSave): Promise<WriteFailure | null> {
    this.started += 1
    const order = this.started
    const answer = await save.write()
    if (answer === "taken") {
      if (order > this.latestTaken) {
        this.latestTaken = order
        this.written = save.value
        save.taken(save.value)
      }
      return null
    }
    return answer === "superseded" ? null : answer
  }
}

/**
 * What the backend answered a write of a field: it took the value; it wrote nothing, as a newer write of the key of the
 * page was decided first (`superseded`); or it failed, which the write told the user already
 */
export type WriteAnswer = "taken" | "superseded" | WriteFailure

/** A write that failed, with its error */
export interface WriteFailure {
  readonly failed: unknown
}

/** A save of a field: its value, its write and what follows a write the backend took */
interface FieldSave {
  readonly value: string
  readonly write: () => Promise<WriteAnswer>
  readonly taken: (value: string) => void
}

/**
 * The id this page drew at its load: a window loaded again, or opened again under the same label, is another page,
 * whatever its host knows of the load, so none of its writes is compared with the numbers of the page before
 */
const PAGE = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`

/** The number of the last write of this page */
let lastWriteSeq = 0

/**
 * What orders the next write of this page (`order` of `set_repository_config` and `set_setting`): the page and the
 * number of the write, from 1 at its load. The writer applies a write of a key only when it is newer than the last
 * write of that key of the page it decided (`sync_utils::page_writes`). One count for every write of the page, so a
 * field that mounts anew never repeats a number of a field before it
 */
export function nextWriteOrder(): WriteOrder {
  lastWriteSeq += 1
  return { page: PAGE, seq: lastWriteSeq }
}
