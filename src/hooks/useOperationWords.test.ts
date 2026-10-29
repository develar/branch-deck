import { describe, expect, it } from "vite-plus/test"
import { namesOperations } from "@/hooks/useOperationWords"
import type { ActionState } from "@/lib/bindings"
import { disabled, enabled } from "@/test-support/actions"

describe("the operations a surface subscribes to", () => {
  it("are none while its items only start, or are disabled for another reason: a step of another operation renders nothing", () => {
    expect(namesOperations([...enabled("approve", "request-changes"), disabled("safe-push-pull-request", { code: "draft", number: 7 })])).toBe(false)
    expect(namesOperations(undefined)).toBe(false)
  })

  it("are the session's while an item of it runs, waits in the queue, waits behind HEAD or is busy behind one", () => {
    const states: ActionState[] = [
      { id: "approve", availability: { kind: "running", op: "op-1" } },
      { id: "approve", availability: { kind: "queued", op: "op-2", behind: "op-1" } },
      { id: "approve", availability: { kind: "waits", op: "op-1" } },
      disabled("approve", { code: "busy", op: "op-1" }),
    ]
    for (const state of states) {
      expect(namesOperations([state])).toBe(true)
    }
  })

  it("names only the items the surface words: a busy action of another surface renders nothing here", () => {
    const actions = [disabled("push", { code: "busy", op: "op-1" }), ...enabled("request-review")]
    expect(namesOperations(actions, ["request-review"])).toBe(false)
    expect(namesOperations(actions, ["push"])).toBe(true)
  })
})
