// @spec openspec/specs/branch/start-in-air/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot } from "@/lib/bindings"
import { startRobotInAir } from "@/lib/robot-prompt-copy"
import { describeToast, openToasts, resetToasts, toasts } from "@/test-support/toast-mock"

const { airOpenDraft, copied, notifyError } = vi.hoisted(() => ({
  airOpenDraft: vi.fn(),
  /** What the clipboard got, in order */
  copied: [] as string[],
  notifyError: vi.fn(),
}))

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/lib/bindings", () => ({
  commands: {
    airOpenDraft,
    agentCliCommand: async () => ({ status: "ok", data: null }),
    getRobotFailure: async () => ({ status: "ok", data: { failure: {} } }),
  },
}))
vi.mock("@/lib/robot-prompt", () => ({ buildRobotPrompt: () => "Fix the Safe Push of auth" }))
vi.mock("@/lib/clipboard", () => ({
  copyTextLater: async (text: Promise<string>) => {
    copied.push(await text)
  },
}))
vi.mock("@/lib/errors", async importOriginal => ({ ...(await importOriginal<typeof import("@/lib/errors")>()), notifyError }))
vi.mock("@/lib/settings-window", () => ({ openSettings: () => {} }))
vi.mock("@/stores/patronus", () => ({ patronusRequest: () => ({ repositoryPath: "/work/idea" }) }))
vi.mock("@/stores/repository", () => ({ getEffectiveBranchPrefix: () => "jane" }))
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master", stackBranches: {} }) },
  structureIn: () => ({ ancestors: [] }),
}))

const ROBOT: PatronusRobot = {
  id: "r1",
  name: "auth",
  status: "failure",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "tip",
  targetBranch: "master",
  startTime: 100,
  finishTime: 200,
  cancellationReason: null,
  url: "https://patronus/robot/r1",
}

const IDE = { product: "IU", build: "IU-271.1", project: "/work/idea" }

beforeEach(() => {
  resetToasts()
  copied.length = 0
  airOpenDraft.mockReset()
  notifyError.mockReset()
})

describe("startRobotInAir", () => {
  it("says it opens while the IDE may ask first, sends the fast prompt alone, then says the IDE opened the New Session tab", async () => {
    let answer: (value: unknown) => void = () => {}
    airOpenDraft.mockReturnValue(new Promise(resolve => (answer = resolve)))
    const started = startRobotInAir(ROBOT)
    expect(openToasts().map(it => [it.type, ...describeToast(it), it.timeout])).toEqual([["loading", "Opening in Air…", "New Session for auth", undefined, undefined, 0]])
    await vi.waitFor(() => expect(airOpenDraft).toHaveBeenCalledWith({ project: "/work/idea", prompt: "Fix the Safe Push of auth" }))
    answer({ status: "ok", data: { status: "opened", ide: IDE } })
    await started
    // the same toast, in place
    expect(toasts.map(it => [it.type, ...describeToast(it)])).toEqual([["success", "Opened in IU-271.1", "New Session for auth", undefined, undefined]])
  })

  it("says no IDE has the repository open, and its Copy Prompt copies the fast prompt", async () => {
    airOpenDraft.mockResolvedValue({ status: "ok", data: { status: "noIde" } })
    await startRobotInAir(ROBOT)
    const [shown] = openToasts()
    expect([shown!.type, ...describeToast(shown!)]).toEqual([
      "info",
      "No IDE with Air has idea open",
      "Open the repository in an IDE with Air and try again, or copy the prompt",
      "Copy Prompt",
      undefined,
    ])
    shown!.actionProps!.onClick()
    await vi.waitFor(() => expect(openToasts().map(it => it.title)).toEqual(["Copied the prompt for auth"]))
    expect(copied).toEqual(["Fix the Safe Push of auth"])
  })

  it("says a refusal of the IDE in its words, and an error of the command as an error", async () => {
    airOpenDraft.mockResolvedValue({ status: "ok", data: { status: "refused", ide: IDE, code: "UNAVAILABLE", message: "No agent of the IDE can start a session." } })
    await startRobotInAir(ROBOT)
    expect(openToasts().map(it => [it.type, ...describeToast(it)])).toEqual([
      [
        "error",
        "Cannot open the New Session for auth in Air",
        "The IDE cannot ask for consent now, has no agent that can start, or did not open the tab in time. Try again with the IDE window open",
        undefined,
        undefined,
      ],
    ])

    resetToasts()
    const failure = { code: "internal", message: "The IDE did not answer within 120 s" }
    airOpenDraft.mockResolvedValue({ status: "error", error: failure })
    await startRobotInAir(ROBOT)
    expect(openToasts()).toEqual([])
    expect(notifyError).toHaveBeenCalledWith("Cannot open the New Session for auth in Air", expect.objectContaining(failure))
  })
})
