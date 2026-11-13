/**
 * The writes of the typing of one field of Settings (`useConfigDraft`): a blur, Enter and the close of the window each
 * save the field, so one change must be one write. A save while a write runs waits for that write (a blur after Enter,
 * a close during the write of a blur). A save of the value a write took writes nothing again until the field dropped its
 * typing (`typingCleared`): the page renders the field without it only after the write answered, and a close of the
 * window between that answer and that render saves the typing of the render before (two writes of one change before,
 * repository-settings.spec.ts "should save valid typing when the window closes, and nothing else").
 */
export class FieldWrites {
  private running: Promise<void> | null = null
  private written: string | null = null

  /**
   * Writes `value` with `write`, which answers whether the backend took it; `taken` runs after a write that was taken.
   * Resolves once the write that serves the save ended, at once when none is needed
   */
  save(value: string, write: () => Promise<boolean>, taken: () => void): Promise<void> {
    if (this.running !== null) {
      return this.running
    }
    if (value === this.written) {
      return Promise.resolve()
    }
    const running = write().then(wasTaken => {
      this.running = null
      if (wasTaken) {
        this.written = value
        taken()
      }
    })
    this.running = running
    return running
  }

  /** The field shows no typing any more: the value it shows is the one written, and a typing anew is written anew */
  typingCleared(): void {
    this.written = null
  }
}
