/**
 * The writes of the typing of one field of Settings (`useConfigDraft`): a blur, Enter and the close of the window each
 * save the field, so one change must be one write, and no change may be lost. A save while a write runs waits for that
 * write and then writes its own value, the latest of those saves, unless it is the value that write took or refused (a
 * blur after Enter, a close during the write of a blur write nothing more; an edit typed during the write is written
 * after it). A save of the value a write took writes nothing again until the field dropped its typing (`typingCleared`):
 * the page renders the field without it only after the write answered, and a close of the window between that answer
 * and that render saves the typing of the render before (two writes of one change before, repository-settings.spec.ts
 * "should save valid typing when the window closes, and nothing else").
 */
export class FieldWrites {
  private running: Promise<void> | null = null
  private written: string | null = null
  /** The latest save that came while a write ran: written once that write ended */
  private next: FieldSave | null = null

  /**
   * Writes `value` with `write`, which answers whether the backend took it; `taken` runs with the value after a write
   * that took it (the field drops its typing only while the typing is that value). Resolves once the writes that serve
   * the save ended, at once when none is needed
   */
  save(value: string, write: () => Promise<boolean>, taken: (value: string) => void): Promise<void> {
    if (this.running !== null) {
      this.next = { value, write, taken }
      return this.running
    }
    if (value === this.written) {
      return Promise.resolve()
    }
    const running = this.run({ value, write, taken })
    this.running = running
    return running
  }

  /** The field shows no typing any more: the value it shows is the one written, and a typing anew is written anew */
  typingCleared(): void {
    this.written = null
  }

  /** Writes `first`, then each latest save that came meanwhile with another value */
  private async run(first: FieldSave): Promise<void> {
    let current: FieldSave | null = first
    while (current !== null) {
      const save: FieldSave = current
      if (await save.write()) {
        this.written = save.value
        save.taken(save.value)
      }
      const next: FieldSave | null = this.next
      this.next = null
      current = next !== null && next.value !== save.value ? next : null
    }
    this.running = null
  }
}

/** A save of a field: its value, its write and what follows a write the backend took */
interface FieldSave {
  readonly value: string
  readonly write: () => Promise<boolean>
  readonly taken: (value: string) => void
}
