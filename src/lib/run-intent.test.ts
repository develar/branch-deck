import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Intent, Plan, Version } from "@/lib/bindings"
import { runFailure, runIntent } from "@/lib/run-intent"

const { engine } = vi.hoisted(() => ({
  engine: {
    /** What reached Rust, in order */
    calls: [] as string[],
    /** The plan `prepare` answers; null: none */
    plan: null as Plan | null,
    /** The versions the page waited for */
    waited: [] as string[],
    /** What Activity recorded */
    records: [] as string[],
  },
}))

const VERSION: Version = { epoch: 1, seq: 7 }

// a channel of the page hears the events of the run; here nobody sends any
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {}
  },
}))
vi.mock("@/lib/bindings", () => ({
  commands: {
    prepare: async (params: { repositoryPath: string; intent: Intent }) => {
      engine.calls.push(`prepare ${params.intent.kind} in ${params.repositoryPath}`)
      return { status: "ok", data: engine.plan }
    },
    execute: async (params: { repositoryPath: string; fingerprint: string }) => {
      engine.calls.push(`execute ${params.fingerprint} in ${params.repositoryPath}`)
      const effects = engine.plan!.effects.map(effect => ({ effect, status: { kind: "done" } }))
      return { status: "ok", data: { op: "op", effects, newsKey: null, versionAfter: VERSION, result: { kind: "none" }, stats: {} } }
    },
  },
}))
vi.mock("@/model/store", () => ({
  whenSessionViewAt: async (repositoryPath: string, version: Version | null) => {
    engine.waited.push(`${repositoryPath} at ${version?.seq}`)
  },
}))
vi.mock("@/stores/activity", () => ({
  recordRetarget: (title: string, description: string) => engine.records.push(`${title}: ${description}`),
  recordPush: (_repositoryPath: string, name: string) => engine.records.push(`push ${name}`),
}))
vi.mock("@/stores/branch-sync", () => ({ getBranch: () => undefined }))
vi.mock("@/stores/pull-requests", () => ({ getPullRequest: () => ({ url: "https://github.com/acme/monorepo/pull/7" }) }))
vi.mock("@/stores/repository", () => ({ getEffectiveBranchPrefix: () => "user" }))

const RETARGET: Intent = { kind: "retarget", number: 7 }

function plan(patch: Partial<Plan>): Plan {
  return {
    intent: RETARGET,
    fingerprint: "f1",
    lands: [],
    effects: [{ kind: "retarget", branch: "auth", number: 7, from: "user/virtual/core", to: "master" }],
    confirmations: [],
    blockers: [],
    waits: [],
    alternatives: null,
    pullRequest: 7,
    approvalsSkippedBy: null,
    ...patch,
  }
}

beforeEach(() => {
  engine.calls.length = 0
  engine.waited.length = 0
  engine.records.length = 0
})

describe("runIntent", () => {
  it("runs the plan Rust prepared on its fingerprint, then waits for the view of what it changed and records its effects", async () => {
    engine.plan = plan({})
    const run = await runIntent("/repo", RETARGET)
    expect(run.kind).toBe("ran")
    expect(engine.calls).toEqual(["prepare retarget in /repo", "execute f1 in /repo"])
    expect(engine.waited).toEqual(["/repo at 7"])
    expect(engine.records).toEqual(["Retargeted #7 to master: auth · it went into core"])
    expect(runFailure(run, "none")).toBeNull()
  })

  it("runs nothing its plan refuses, nor without a plan, and words why", async () => {
    engine.plan = plan({ blockers: [{ code: "pullRequestMerged", number: 7 }] })
    const refused = await runIntent("/repo", RETARGET)
    expect(refused.kind).toBe("refused")
    expect(runFailure(refused, "none")).toBe("Pull request #7 is merged")
    engine.plan = null
    expect(runFailure(await runIntent("/repo", RETARGET), "No pull request #7 is listed")).toBe("No pull request #7 is listed")
    expect(engine.calls).toEqual(["prepare retarget in /repo", "prepare retarget in /repo"])
  })
})
