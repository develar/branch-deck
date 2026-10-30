import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { AirLaunchChoice, AirLaunchForm } from "@/lib/air-launch"
import {
  choiceOf,
  effortsOf,
  findTarget,
  formatEffort,
  formatModel,
  formatRefusal,
  formatStartHint,
  initialForm,
  launchInAir,
  launchRequest,
  readAirTargets,
  targetKey,
  withModel,
  withTarget,
} from "@/lib/air-launch"
import type { AirTarget } from "@/lib/bindings"

const { airLaunch, airLaunchTargets } = vi.hoisted(() => ({ airLaunch: vi.fn(), airLaunchTargets: vi.fn() }))
vi.mock("@/lib/bindings", () => ({ commands: { airLaunch, airLaunchTargets } }))

const CODEX: AirTarget = {
  agentId: "codex",
  launchTargetId: "codex",
  interactionSurfaceId: "native",
  name: "Codex",
  models: [
    { id: "gpt-5.4", name: "GPT-5.4", qualifier: null, isDefault: false, reasoningEfforts: ["low", "medium"] },
    { id: "gpt-5.5", name: "GPT-5.5", qualifier: "preview", isDefault: true, reasoningEfforts: ["low", "medium", "high", "xhigh"] },
  ],
}
const CLAUDE: AirTarget = { agentId: "claude", launchTargetId: "claude", interactionSurfaceId: "acp", name: "Claude", models: [] }
const TARGETS = [CODEX, CLAUDE]

function form(overrides: Partial<AirLaunchForm> = {}): AirLaunchForm {
  return { target: targetKey(CODEX), model: "gpt-5.5", effort: null, start: "run", ...overrides }
}

describe("initialForm", () => {
  it("starts with the first agent, its default model, the default effort and Run", () => {
    expect(initialForm(TARGETS, undefined)).toEqual(form())
  })

  it("starts with the remembered agent, model, effort and start while the IDE still offers them", () => {
    const remembered: AirLaunchChoice = { launchTargetId: "codex", interactionSurfaceId: "native", model: "gpt-5.4", effort: "medium", start: "draft" }
    expect(initialForm(TARGETS, remembered)).toEqual(form({ model: "gpt-5.4", effort: "medium", start: "draft" }))
  })

  it("drops a remembered model or effort the agent no longer offers", () => {
    const remembered: AirLaunchChoice = { launchTargetId: "codex", interactionSurfaceId: "native", model: "gpt-4", effort: "xhigh", start: "run" }
    // the default model takes xhigh
    expect(initialForm(TARGETS, remembered)).toEqual(form({ effort: "xhigh" }))
    const lowOnly: AirLaunchChoice = { ...remembered, model: "gpt-5.4" }
    expect(initialForm(TARGETS, lowOnly)).toEqual(form({ model: "gpt-5.4", effort: null }))
  })

  it("takes the first agent when the remembered one is gone, and keeps the remembered start", () => {
    const remembered: AirLaunchChoice = { launchTargetId: "gemini", interactionSurfaceId: "acp", model: "g", effort: "high", start: "draft" }
    expect(initialForm(TARGETS, remembered)).toEqual(form({ start: "draft" }))
  })

  it("has no model and no effort for an agent without models, and no form without agents", () => {
    expect(initialForm([CLAUDE], undefined)).toEqual({ target: targetKey(CLAUDE), model: null, effort: null, start: "run" })
    expect(initialForm([], undefined)).toBeNull()
  })
})

describe("the pickers", () => {
  it("offers the efforts of the chosen model, none for an agent without models", () => {
    expect(effortsOf(CODEX, "gpt-5.4")).toEqual(["low", "medium"])
    expect(effortsOf(CLAUDE, null)).toEqual([])
  })

  it("another agent takes its default model and keeps the effort only while that model takes it", () => {
    expect(withTarget(form({ effort: "high" }), TARGETS, targetKey(CLAUDE))).toEqual({ target: targetKey(CLAUDE), model: null, effort: null, start: "run" })
    const back = withTarget({ target: targetKey(CLAUDE), model: null, effort: null, start: "draft" }, TARGETS, targetKey(CODEX))
    expect(back).toEqual(form({ start: "draft" }))
    expect(withTarget(form(), TARGETS, "gone")).toEqual(form())
  })

  it("another model keeps the effort only while it takes it", () => {
    expect(withModel(form({ effort: "medium" }), CODEX, "gpt-5.4")).toEqual(form({ model: "gpt-5.4", effort: "medium" }))
    expect(withModel(form({ effort: "xhigh" }), CODEX, "gpt-5.4")).toEqual(form({ model: "gpt-5.4", effort: null }))
  })

  it("names a model with its qualifier and an effort by its word", () => {
    expect(formatModel(CODEX.models![1]!)).toBe("GPT-5.5 (preview)")
    expect(formatModel(CODEX.models![0]!)).toBe("GPT-5.4")
    expect(formatEffort("xhigh")).toBe("Extra High")
    expect(formatEffort("turbo")).toBe("turbo")
  })
})

describe("the launch", () => {
  it("sends the target, the model, the effort, the prompt and the title of the form", () => {
    expect(launchRequest("/work/idea", form({ effort: "high" }), CODEX, "Fix it", "Fix the Safe Push of auth")).toEqual({
      project: "/work/idea",
      launchTargetId: "codex",
      interactionSurfaceId: "native",
      model: "gpt-5.5",
      effort: "high",
      prompt: "Fix it",
      title: "Fix the Safe Push of auth",
      start: "run",
    })
  })

  it("sends no blank prompt of a draft", () => {
    expect(launchRequest("/work/idea", form({ start: "draft" }), CODEX, "  ", "t").prompt).toBeNull()
  })

  it("remembers the target, the model, the effort and the start", () => {
    expect(choiceOf(form({ effort: "low", start: "draft" }), CODEX)).toEqual({
      launchTargetId: "codex",
      interactionSurfaceId: "native",
      model: "gpt-5.5",
      effort: "low",
      start: "draft",
    })
  })

  it("runs no blank prompt, and sends again only after a change of what the IDE refused", () => {
    expect(formatStartHint(form(), " \n", null)).toBe("Write a prompt to run the session, or start a draft")
    expect(formatStartHint(form({ start: "draft" }), "", null)).toBeNull()
    const refusal = { code: "MODEL_UNAVAILABLE", message: null, form: form(), prompt: "Fix it" }
    expect(formatStartHint(form(), "Fix it", refusal)).toBe("Change the agent, the model, the effort or the prompt to try again")
    expect(formatStartHint(form({ model: "gpt-5.4" }), "Fix it", refusal)).toBeNull()
    expect(formatStartHint(form(), "Fix it now", refusal)).toBeNull()
  })

  it("finds a target by its key: two targets of one agent on two surfaces are two", () => {
    const other: AirTarget = { ...CODEX, interactionSurfaceId: "acp" }
    expect(findTarget([CODEX, other], targetKey(other))).toBe(other)
  })
})

describe("formatRefusal", () => {
  it("words every code of protocol v1, the message of a failed launch, and an unknown code with its message", () => {
    expect(formatRefusal("DENIED", "user said no")).toBe("The IDE denied Branch Deck, or nobody answered its question in time")
    expect(formatRefusal("UNTRUSTED_PROJECT", null)).toBe("The IDE does not trust the project. Trust it in the IDE first")
    expect(formatRefusal("LAUNCH_FAILED", "codex exited with 1")).toBe("The session did not start: codex exited with 1")
    expect(formatRefusal("QUOTA", "later")).toBe("The IDE refused the request (QUOTA): later")
    expect(formatRefusal("QUOTA", null)).toBe("The IDE refused the request (QUOTA)")
    const codes = [
      "DENIED",
      "UNAVAILABLE",
      "BUSY",
      "RATE_LIMITED",
      "UNSUPPORTED_VERSION",
      "UNKNOWN_METHOD",
      "BAD_REQUEST",
      "NO_PROJECT",
      "UNTRUSTED_PROJECT",
      "TARGET_UNAVAILABLE",
      "MODEL_UNAVAILABLE",
      "MODEL_AMBIGUOUS",
      "INVALID_REASONING_EFFORT",
      "EMPTY_PROMPT",
      "LAUNCH_FAILED",
    ]
    for (const code of codes) {
      expect(formatRefusal(code, null)).not.toContain(code)
    }
  })
})

describe("the commands", () => {
  beforeEach(() => {
    airLaunch.mockReset()
    airLaunchTargets.mockReset()
  })

  it("answers the targets and the launch, and throws an error of the command with its code", async () => {
    airLaunchTargets.mockResolvedValue({ status: "ok", data: { status: "noIde" } })
    await expect(readAirTargets("/work/idea")).resolves.toEqual({ status: "noIde" })
    expect(airLaunchTargets).toHaveBeenCalledWith({ project: "/work/idea" })

    airLaunch.mockResolvedValue({ status: "error", error: { code: "internal", message: "The IDE did not answer within 120 s" } })
    const request = launchRequest("/work/idea", form(), CODEX, "Fix it", "t")
    await expect(launchInAir(request)).rejects.toMatchObject({ code: "internal", message: "The IDE did not answer within 120 s" })
    expect(airLaunch).toHaveBeenCalledWith(request)
  })
})
