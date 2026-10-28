import { describe, expect, it } from "vite-plus/test"
import { createStore } from "zustand/vanilla"
import { askUntilSettled } from "@/lib/ask-until-settled"

/** A fact as a store has it: unknown (null), then its answer */
function factStore() {
  return createStore<{ fact: string | null }>(() => ({ fact: null }))
}

/** Lets every queued promise continuation run */
async function settle(): Promise<void> {
  for (let i = 0; i < 10; i++) {
    await Promise.resolve()
  }
}

describe("askUntilSettled", () => {
  it("asks again at once when the fact came while it asked, ahead of the answer that still waits for it", async () => {
    const store = factStore()
    const answers: string[] = []
    const answer = await askUntilSettled(
      () => {
        const answered = store.getState().fact ?? "wait"
        // the patch of the fact reaches the page before the answer of the ask the session made without it
        store.setState({ fact: "ready" })
        answers.push(answered)
        return Promise.resolve(answered)
      },
      answered => answered !== "wait",
      [store],
    )
    expect(answer).toBe("ready")
    expect(answers).toEqual(["wait", "ready"])
  })

  it("waits for the next change of any source when nothing changed, then asks again", async () => {
    const other = factStore()
    const store = factStore()
    let asks = 0
    let settled: string | undefined
    void askUntilSettled(
      () => {
        asks++
        return Promise.resolve(store.getState().fact ?? "wait")
      },
      answered => answered !== "wait",
      [other, store],
    ).then(answer => (settled = answer))
    await settle()
    expect(asks).toBe(1)
    expect(settled).toBeUndefined()
    store.setState({ fact: "ready" })
    await settle()
    expect(asks).toBe(2)
    expect(settled).toBe("ready")
    // settled: a later change asks nothing
    other.setState({ fact: "later" })
    await settle()
    expect(asks).toBe(2)
  })

  it("stops watching when the ask fails", async () => {
    const store = factStore()
    let asks = 0
    await expect(
      askUntilSettled(
        () => {
          asks++
          return Promise.reject(new Error("no session"))
        },
        () => false,
        [store],
      ),
    ).rejects.toThrow("no session")
    store.setState({ fact: "ready" })
    await settle()
    expect(asks).toBe(1)
  })
})
