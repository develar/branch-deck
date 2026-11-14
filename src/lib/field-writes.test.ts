// The writes of the typing of a field of Settings (src/lib/field-writes.ts): one change is one write, whichever of a
// blur, Enter and the close of the window saves it
import { describe, expect, it, vi } from "vite-plus/test"
import { FieldWrites } from "@/lib/field-writes"

/** A write the test answers: `answer(true)` takes it */
function heldWrite(): { write: () => Promise<boolean>; calls: () => number; answer: (taken: boolean) => void } {
  let calls = 0
  let answer: (taken: boolean) => void = () => {}
  return {
    write: () => {
      calls++
      return new Promise<boolean>(resolve => {
        answer = resolve
      })
    },
    calls: () => calls,
    answer: taken => answer(taken),
  }
}

describe("the writes of the typing of a field", () => {
  it("writes a change once when the window closes after the write of its blur answered and before the field dropped its typing", async () => {
    const writes = new FieldWrites()
    const held = heldWrite()
    let taken = 0
    const blur = writes.save("acme/app", held.write, () => taken++)
    held.answer(true)
    await blur
    expect(taken).toBe(1)
    // the close saves the typing of the render before the answer, which still holds it
    await writes.save("acme/app", held.write, () => taken++)
    expect(held.calls()).toBe(1)
  })

  it("lets a save of the same value during a write wait for it, and writes nothing more", async () => {
    const writes = new FieldWrites()
    const held = heldWrite()
    const enter = writes.save("acme/app", held.write, () => {})
    let closed = false
    const close = writes
      .save("acme/app", held.write, () => {})
      .then(() => {
        closed = true
      })
    await Promise.resolve()
    expect(closed).toBe(false)
    held.answer(true)
    await Promise.all([enter, close])
    expect(closed).toBe(true)
    expect(held.calls()).toBe(1)
  })

  it("writes an edit saved during a write after it, the latest of such saves, and the save waits for both", async () => {
    const writes = new FieldWrites()
    const written: string[] = []
    const taken: string[] = []
    let answer: (taken: boolean) => void = () => {}
    const write = (value: string) => () => {
      written.push(value)
      return new Promise<boolean>(resolve => {
        answer = resolve
      })
    }
    const blur = writes.save("acme/one", write("acme/one"), value => taken.push(value))
    // typed while the write of the blur runs: Enter, then the close of the window, each with a later edit
    void writes.save("acme/two", write("acme/two"), value => taken.push(value))
    let closed = false
    const close = writes
      .save("acme/three", write("acme/three"), value => taken.push(value))
      .then(() => {
        closed = true
      })
    answer(true)
    await vi.waitFor(() => expect(written).toEqual(["acme/one", "acme/three"]))
    expect(closed).toBe(false)
    answer(true)
    await Promise.all([blur, close])
    expect(closed).toBe(true)
    expect(taken).toEqual(["acme/one", "acme/three"])
  })

  it("writes again a value the backend did not take, and a typing anew once the field dropped the one written", async () => {
    const writes = new FieldWrites()
    const held = heldWrite()
    const refused = writes.save("acme/app", held.write, () => {})
    held.answer(false)
    await refused
    const again = writes.save("acme/app", held.write, () => {})
    held.answer(true)
    await again
    expect(held.calls()).toBe(2)

    writes.typingCleared()
    const anew = writes.save("acme/app", held.write, () => {})
    held.answer(true)
    await anew
    expect(held.calls()).toBe(3)
  })
})
