// The writes of the typing of a field of Settings (src/lib/field-writes.ts): one change is one write, whichever of a
// blur, Enter and the close of the window saves it
import { describe, expect, it, vi } from "vite-plus/test"
import { FieldWrites } from "@/lib/field-writes"

/** Writes the test answers, each recording its value as it starts: `answer(true)` takes the last one */
function recordedWrites(): { written: string[]; write: (value: string) => () => Promise<boolean>; answer: (taken: boolean) => void } {
  const written: string[] = []
  let last: (taken: boolean) => void = () => {}
  return {
    written,
    write: value => () => {
      written.push(value)
      return new Promise<boolean>(resolve => {
        last = resolve
      })
    },
    answer: taken => last(taken),
  }
}

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
    const blur = writes.save("acme/app", "", held.write, () => taken++)
    held.answer(true)
    await blur
    expect(taken).toBe(1)
    // the close saves the typing of the render before the answer, which still holds it
    await writes.save("acme/app", "", held.write, () => taken++)
    expect(held.calls()).toBe(1)
  })

  it("lets a save of the same value during a write wait for it, and writes nothing more", async () => {
    const writes = new FieldWrites()
    const held = heldWrite()
    const enter = writes.save("acme/app", "", held.write, () => {})
    let closed = false
    const close = writes
      .save("acme/app", "", held.write, () => {})
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
    const { written, write, answer } = recordedWrites()
    const taken: string[] = []
    const blur = writes.save("acme/one", "", write("acme/one"), value => taken.push(value))
    // typed while the write of the blur runs: Enter, then the close of the window, each with a later edit
    void writes.save("acme/two", "", write("acme/two"), value => taken.push(value))
    let closed = false
    const close = writes
      .save("acme/three", "", write("acme/three"), value => taken.push(value))
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
    const refused = writes.save("acme/app", "", held.write, () => {})
    held.answer(false)
    await refused
    const again = writes.save("acme/app", "", held.write, () => {})
    held.answer(true)
    await again
    expect(held.calls()).toBe(2)

    writes.typingCleared()
    const anew = writes.save("acme/app", "", held.write, () => {})
    held.answer(true)
    await anew
    expect(held.calls()).toBe(3)
  })

  it("writes a value saved during a write back to the one the field showed before it, which the field still shows then", async () => {
    const writes = new FieldWrites()
    const { written, write, answer } = recordedWrites()
    // Enter writes acme/one; the field is emptied and Enter pressed again before the answer, the field still showing ""
    const enter = writes.save("acme/one", "", write("acme/one"), () => {})
    const back = writes.save("", "", write(""), () => {})
    answer(true)
    await vi.waitFor(() => expect(written).toEqual(["acme/one", ""]))
    answer(true)
    await Promise.all([enter, back])
    // with no write running, the value the field shows writes nothing and drops the typing
    const dropped: string[] = []
    await writes.save("", "", write("never"), value => dropped.push(value))
    expect(written).toEqual(["acme/one", ""])
    expect(dropped).toEqual([""])
  })

  it("writes again after a write that threw: the save rejects with its error, and the next save of the field writes", async () => {
    const writes = new FieldWrites()
    await expect(
      writes.save(
        "acme/app",
        "",
        () => Promise.reject(new Error("the link broke")),
        () => {},
      ),
    ).rejects.toThrow("the link broke")
    const held = heldWrite()
    const again = writes.save("acme/app", "", held.write, () => {})
    held.answer(true)
    await again
    expect(held.calls()).toBe(1)
  })
})
