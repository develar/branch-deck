import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { AirLaunchChoice, AirLaunchForm } from "@/lib/air-launch"
import {
  choiceOf,
  effortsOf,
  findTarget,
  formatEffort,
  formatModel,
  formatRefusal,
  formatSessionStatus,
  formatStartHint,
  formatWatchEnd,
  initialForm,
  isModeLocked,
  launchInAir,
  launchRequest,
  modesOf,
  readAirTargets,
  stopAirWatch,
  targetKey,
  watchAirSession,
  withModel,
  withPlan,
  withTarget,
} from "@/lib/air-launch"
import type { AirTarget } from "@/lib/bindings"

const { airLaunch, airLaunchTargets, airWatchSession, stopAirWatchCommand } = vi.hoisted(() => ({
  airLaunch: vi.fn(),
  airLaunchTargets: vi.fn(),
  airWatchSession: vi.fn(),
  stopAirWatchCommand: vi.fn(),
}))
vi.mock("@/lib/bindings", () => ({ commands: { airLaunch, airLaunchTargets, airWatchSession, stopAirWatch: stopAirWatchCommand } }))
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

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

/** Codex whose Plan is a mode, beside two other modes */
const CODEX_MODES: AirTarget = {
  ...CODEX,
  modes: [
    { id: "plan", name: "Plan", kind: "plan" },
    { id: "auto", name: "Auto", kind: "auto_review" },
    { id: "full", name: "Full Access", kind: "full_access" },
  ],
  plan: true,
}
/** Claude whose Plan is not a mode */
const CLAUDE_PLAN: AirTarget = { ...CLAUDE, modes: [{ id: "accept", name: "Accept Edits", kind: "standard" }], plan: true }

function form(overrides: Partial<AirLaunchForm> = {}): AirLaunchForm {
  return { target: targetKey(CODEX), model: "gpt-5.5", effort: null, mode: null, plan: false, planEffort: null, start: "run", ...overrides }
}

function claudeForm(overrides: Partial<AirLaunchForm> = {}): AirLaunchForm {
  return { target: targetKey(CLAUDE), model: null, effort: null, mode: null, plan: false, planEffort: null, start: "run", ...overrides }
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
    expect(initialForm([CLAUDE], undefined)).toEqual(claudeForm())
    expect(initialForm([], undefined)).toBeNull()
  })
})

describe("the pickers", () => {
  it("offers the efforts of the chosen model, none for an agent without models", () => {
    expect(effortsOf(CODEX, "gpt-5.4")).toEqual(["low", "medium"])
    expect(effortsOf(CLAUDE, null)).toEqual([])
  })

  it("another agent takes its default model and keeps the effort only while that model takes it", () => {
    expect(withTarget(form({ effort: "high" }), TARGETS, targetKey(CLAUDE))).toEqual(claudeForm())
    const back = withTarget(claudeForm({ start: "draft" }), TARGETS, targetKey(CODEX))
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

describe("the mode and Plan", () => {
  it("offers the modes without the mode of Plan, and Plan only for a target that can start in it", () => {
    expect(modesOf(CODEX_MODES).map(it => it.id)).toEqual(["auto", "full"])
    expect(modesOf(CODEX)).toEqual([])
    expect(withPlan(form(), CODEX, true)).toEqual(form())
    expect(withPlan(form(), CODEX_MODES, true)).toEqual(form({ plan: true }))
  })

  it("starts with the remembered mode, Plan and plan effort while the IDE still offers them", () => {
    const remembered: AirLaunchChoice = {
      launchTargetId: "codex",
      interactionSurfaceId: "native",
      model: "gpt-5.5",
      effort: null,
      mode: "auto",
      plan: true,
      planEffort: "xhigh",
      start: "run",
    }
    expect(initialForm([CODEX_MODES], remembered)).toEqual(form({ mode: "auto", plan: true, planEffort: "xhigh" }))
    // the IDE no longer offers the mode and Plan
    expect(initialForm([{ ...CODEX_MODES, modes: [], plan: false }], remembered)).toEqual(form())
    // the remembered model no longer takes the plan effort
    expect(initialForm([CODEX_MODES], { ...remembered, model: "gpt-5.4" })).toEqual(form({ model: "gpt-5.4", mode: "auto", plan: true }))
    // a choice of an older version has no mode, Plan and plan effort
    const older: AirLaunchChoice = { launchTargetId: "codex", interactionSurfaceId: "native", model: "gpt-5.5", effort: null, start: "run" }
    expect(initialForm([CODEX_MODES], older)).toEqual(form())
  })

  it("keeps the plan effort only with Plan and while the model takes it", () => {
    const planned = form({ plan: true, planEffort: "high" })
    expect(withModel(planned, CODEX_MODES, "gpt-5.4")).toEqual(form({ model: "gpt-5.4", plan: true }))
    expect(withPlan(planned, CODEX_MODES, false)).toEqual(form())
    expect(withPlan(form(), CODEX_MODES, true).planEffort).toBeNull()
    expect(launchRequest("/w", form({ planEffort: "high" }), CODEX_MODES, "Fix it", "t").planEffort).toBeNull()
    expect(launchRequest("/w", planned, CODEX_MODES, "Fix it", "t")).toMatchObject({ plan: true, planEffort: "high" })
  })

  it("another agent keeps the mode and Plan only while it offers them", () => {
    const targets = [CODEX_MODES, CLAUDE_PLAN, CLAUDE]
    expect(withTarget(form({ mode: "auto", plan: true }), targets, targetKey(CLAUDE_PLAN))).toEqual(claudeForm({ plan: true }))
    expect(withTarget(form({ mode: "auto", plan: true }), [CODEX_MODES, CLAUDE], targetKey(CLAUDE))).toEqual(claudeForm())
    expect(withTarget(claudeForm({ mode: "accept" }), [CLAUDE_PLAN, CODEX_MODES], targetKey(CLAUDE_PLAN))).toEqual(claudeForm({ mode: "accept" }))
  })

  it("sends no other mode with Plan of a mode, and the mode with Plan of a target whose Plan is not a mode", () => {
    const planned = form({ mode: "auto", plan: true })
    expect(isModeLocked(planned, CODEX_MODES)).toBe(true)
    expect(launchRequest("/w", planned, CODEX_MODES, "Fix it", "t")).toMatchObject({ mode: null, plan: true })
    expect(launchRequest("/w", form({ mode: "auto" }), CODEX_MODES, "Fix it", "t")).toMatchObject({ mode: "auto", plan: false })
    const claude = claudeForm({ mode: "accept", plan: true })
    expect(isModeLocked(claude, CLAUDE_PLAN)).toBe(false)
    expect(launchRequest("/w", claude, CLAUDE_PLAN, "Fix it", "t")).toMatchObject({ mode: "accept", plan: true })
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
      mode: null,
      plan: false,
      planEffort: null,
      prompt: "Fix it",
      title: "Fix the Safe Push of auth",
      start: "run",
    })
  })

  it("sends no blank prompt of a draft", () => {
    expect(launchRequest("/work/idea", form({ start: "draft" }), CODEX, "  ", "t").prompt).toBeNull()
  })

  it("remembers the target, the model, the effort, the mode, Plan, the plan effort and the start", () => {
    expect(choiceOf(form({ effort: "low", mode: "auto", plan: true, planEffort: "high", start: "draft" }), CODEX_MODES)).toEqual({
      launchTargetId: "codex",
      interactionSurfaceId: "native",
      model: "gpt-5.5",
      effort: "low",
      mode: "auto",
      plan: true,
      planEffort: "high",
      start: "draft",
    })
  })

  it("runs no blank prompt, and sends again only after a change of what the IDE refused", () => {
    expect(formatStartHint(form(), " \n", null)).toBe("Write a prompt to run the session, or start a draft")
    expect(formatStartHint(form({ start: "draft" }), "", null)).toBeNull()
    const refusal = { code: "MODEL_UNAVAILABLE", message: null, form: form(), prompt: "Fix it" }
    expect(formatStartHint(form(), "Fix it", refusal)).toBe("Change a choice of the agent or the prompt to try again")
    expect(formatStartHint(form({ model: "gpt-5.4" }), "Fix it", refusal)).toBeNull()
    expect(formatStartHint(form({ plan: true }), "Fix it", refusal)).toBeNull()
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
    expect(formatRefusal("MODE_CONFLICT", "plan and auto")).toBe("The agent cannot start in Plan with another mode")
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
      "MODE_UNAVAILABLE",
      "MODE_CONFLICT",
      "PLAN_UNAVAILABLE",
      "INVALID_PLAN_REASONING_EFFORT",
      "NO_SESSION",
    ]
    for (const code of codes) {
      expect(formatRefusal(code, null)).not.toContain(code)
    }
  })
})

describe("the status of a session", () => {
  it("words the activity as working, waiting for the user or done, and a later activity as working", () => {
    expect(formatSessionStatus("Codex", "processing")).toEqual({ type: "loading", title: "Codex works in Air…", done: false })
    expect(formatSessionStatus("Codex", "needs_input")).toEqual({ type: "info", title: "Codex waits for you in Air", done: false })
    expect(formatSessionStatus("Codex", "reviewing")).toEqual({ type: "info", title: "Codex waits for your review in Air", done: false })
    expect(formatSessionStatus("Codex", "ready")).toEqual({ type: "success", title: "Codex is done in Air", done: true })
    expect(formatSessionStatus("Codex", "unread").done).toBe(true)
    expect(formatSessionStatus("Codex", "later").type).toBe("loading")
  })

  it("words the end of a watch, and nothing for a watch the page stopped", () => {
    expect(formatWatchEnd("Codex", { status: "stopped" })).toBeNull()
    expect(formatWatchEnd("Codex", { status: "refused", code: "NO_SESSION", message: "gone" })).toEqual({ type: "info", title: "The session of Codex is gone in Air" })
    expect(formatWatchEnd("Codex", { status: "refused", code: "BUSY", message: null })).toEqual({
      type: "warning",
      title: "Cannot follow Codex in Air",
      description: "The IDE answers another request now. Try again in a moment",
    })
    expect(formatWatchEnd("Codex", { status: "ended", reason: "timeout" })?.description).toBe("The IDE ends a watch after 4 hours")
    expect(formatWatchEnd("Codex", { status: "closed" })?.description).toBe("The IDE closed the connection")
  })
})

describe("the commands", () => {
  beforeEach(() => {
    airLaunch.mockReset()
    airLaunchTargets.mockReset()
    airWatchSession.mockReset()
    stopAirWatchCommand.mockReset()
  })

  it("watches a session: each status to the callback, then the end; the stop names the session", async () => {
    airWatchSession.mockImplementation(async (_params: unknown, statuses: { onmessage: (status: unknown) => void }) => {
      statuses.onmessage({ title: "Fix it", activity: "processing" })
      statuses.onmessage({ title: null, activity: "ready" })
      return { status: "ok", data: { status: "stopped" } }
    })
    const seen: unknown[] = []
    await expect(watchAirSession("/work/idea", "s-1", status => seen.push(status))).resolves.toEqual({ status: "stopped" })
    expect(airWatchSession).toHaveBeenCalledWith({ project: "/work/idea", sessionId: "s-1" }, expect.anything())
    expect(seen).toEqual([
      { title: "Fix it", activity: "processing" },
      { title: null, activity: "ready" },
    ])
    stopAirWatch("s-1")
    expect(stopAirWatchCommand).toHaveBeenCalledWith("s-1")

    airWatchSession.mockResolvedValue({ status: "error", error: { code: "internal", message: "Cannot connect to the IDE" } })
    await expect(watchAirSession("/work/idea", "s-1", () => {})).rejects.toMatchObject({ code: "internal" })
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
