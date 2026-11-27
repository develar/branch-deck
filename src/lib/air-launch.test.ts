import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { formatIde, formatRefusal, openDraftInAir } from "@/lib/air-launch"

const { airOpenDraft } = vi.hoisted(() => ({ airOpenDraft: vi.fn() }))
vi.mock("@/lib/bindings", () => ({ commands: { airOpenDraft } }))

describe("formatRefusal", () => {
  it("words every refusal code of a draft, the message of a failed open, and an unknown code with its message", () => {
    expect(formatRefusal("DENIED", "user said no")).toBe("The IDE denied Branch Deck, or nobody answered its question in time")
    expect(formatRefusal("NO_DEFAULT", null)).toBe("The New Session button of the IDE has no default agent. Pick an agent there, then try again")
    expect(formatRefusal("UNTRUSTED_PROJECT", null)).toBe("The IDE does not trust the project. Trust it in the IDE first")
    expect(formatRefusal("LAUNCH_FAILED", "the agent is gone")).toBe("The IDE cannot open the New Session tab: the agent is gone")
    expect(formatRefusal("QUOTA", "later")).toBe("The IDE refused the request (QUOTA): later")
    expect(formatRefusal("QUOTA", null)).toBe("The IDE refused the request (QUOTA)")
    const codes = [
      "DENIED",
      "UNAVAILABLE",
      "BUSY",
      "RATE_LIMITED",
      "UNAUTHORIZED",
      "UNKNOWN_METHOD",
      "BAD_REQUEST",
      "BAD_PARAMS",
      "NO_PROJECT",
      "NO_DEFAULT",
      "UNTRUSTED_PROJECT",
      "TARGET_UNAVAILABLE",
      "UNKNOWN_AGENT",
      "LAUNCH_FAILED",
    ]
    for (const code of codes) {
      expect(formatRefusal(code, null)).not.toContain(code)
    }
  })

  it("names the IDE by its build, else by its product", () => {
    expect(formatIde({ product: "IU", build: "IU-271.1" })).toBe("IU-271.1")
    expect(formatIde({ product: "IU", build: "" })).toBe("IU")
  })
})

describe("the command", () => {
  beforeEach(() => {
    airOpenDraft.mockReset()
  })

  it("sends the project and the prompt alone, answers the answer of the IDE, and throws an error of the command with its code", async () => {
    const ide = { product: "IU", build: "IU-271.1", project: "/work/idea" }
    airOpenDraft.mockResolvedValue({ status: "ok", data: { status: "opened", ide } })
    await expect(openDraftInAir("/work/idea", "Fix it")).resolves.toEqual({ status: "opened", ide })
    expect(airOpenDraft).toHaveBeenCalledWith({ project: "/work/idea", prompt: "Fix it" })

    airOpenDraft.mockResolvedValue({ status: "error", error: { code: "internal", message: "The IDE did not answer within 120 s" } })
    await expect(openDraftInAir("/work/idea", "Fix it")).rejects.toMatchObject({ code: "internal", message: "The IDE did not answer within 120 s" })
  })
})
