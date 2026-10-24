/**
 * One Go to Declaration or Find Usages in flight per view (spec/commit/symbol-navigation.spec.md): a repeat of the
 * same name joins the one in flight (⌘B pressed twice opens one file view), another name replaces it, a cancel drops
 * it (Escape, Cancel), and an answer acts only while the view shows the subject it was asked for (⌥↓ to another commit
 * drops it)
 */
export class NavigationGate {
  private generation = 0
  private inFlight: { key: string; generation: number } | null = null
  private subject: string | null = null

  /** The view shows `subject` now (its revisions): an answer asked for another one is dropped; whether one was in flight */
  show(subject: string): boolean {
    if (this.subject === subject) {
      return false
    }
    this.subject = subject
    return this.cancel()
  }

  /** The navigation in flight is no longer wanted: its answer is dropped; whether one was in flight */
  cancel(): boolean {
    this.generation++
    const dropped = this.inFlight !== null
    this.inFlight = null
    return dropped
  }

  /** The ticket of a navigation of `key` (the action and the name's place), or null when the same one is in flight */
  start(key: string): number | null {
    if (this.inFlight?.key === key) {
      return null
    }
    this.generation++
    this.inFlight = { key, generation: this.generation }
    return this.generation
  }

  /** The answer of `ticket` arrived: whether it may act, no other navigation or subject having come since */
  finish(ticket: number): boolean {
    if (this.inFlight?.generation === ticket) {
      this.inFlight = null
    }
    return ticket === this.generation
  }
}
