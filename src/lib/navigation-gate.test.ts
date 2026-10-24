// @spec spec/commit/symbol-navigation.spec.md
import { describe, expect, it } from "vite-plus/test"
import { NavigationGate } from "@/lib/navigation-gate"

/** A deferred answer of the backend, as `findDeclaration` gives it later */
function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve = () => {}
  const promise = new Promise<void>(done => {
    resolve = done
  })
  return { promise, resolve }
}

describe("NavigationGate", () => {
  it("joins a repeat of the same name: two ⌘B open one view", async () => {
    const gate = new NavigationGate()
    gate.show("c1")
    const opened: number[] = []
    const answer = deferred()
    const press = () => {
      const ticket = gate.start("declaration\nA.kt:3:7")
      if (ticket !== null) {
        void answer.promise.then(() => gate.finish(ticket) && opened.push(ticket))
      }
    }
    press()
    press()
    answer.resolve()
    await answer.promise
    await Promise.resolve()
    expect(opened).toHaveLength(1)
    // once answered, the same name asks again
    expect(gate.start("declaration\nA.kt:3:7")).not.toBeNull()
  })

  it("drops an answer whose subject changed, and one another name replaced", () => {
    const gate = new NavigationGate()
    gate.show("c1")
    const first = gate.start("declaration\nA.kt:3:7")!
    // ⌥↓ shows the next commit before the answer
    gate.show("c2")
    expect(gate.finish(first)).toBe(false)
    const second = gate.start("declaration\nA.kt:3:7")!
    const third = gate.start("usages\nA.kt:3:7")!
    expect(gate.finish(second)).toBe(false)
    expect(gate.finish(third)).toBe(true)
    // the same subject again changes nothing
    gate.show("c2")
    const fourth = gate.start("declaration\nB.kt:1:1")!
    gate.show("c2")
    expect(gate.finish(fourth)).toBe(true)
  })

  it("drops the answer of a canceled navigation, and says whether one was in flight", () => {
    const gate = new NavigationGate()
    expect(gate.show("c1")).toBe(false)
    expect(gate.cancel()).toBe(false)
    const first = gate.start("usages\nA.kt:3:7")!
    // Escape while the index searches: the list never opens, and the same name asks again at once
    expect(gate.cancel()).toBe(true)
    expect(gate.finish(first)).toBe(false)
    const again = gate.start("usages\nA.kt:3:7")!
    expect(again).not.toBeNull()
    // ⌥↓ drops the one in flight and says so, for the backend to stop its search
    expect(gate.show("c2")).toBe(true)
    expect(gate.finish(again)).toBe(false)
    expect(gate.show("c3")).toBe(false)
  })
})
