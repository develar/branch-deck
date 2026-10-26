// @spec spec/branch/pull-requests.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, EffectRecord, Intent, Outcome, Plan, PullRequest, RunningOperation } from "@/lib/bindings"
import type { CreatePullRequestContext } from "@/lib/pull-request-dialog"
import { continueOnGitHub, pullRequestOperationIn, requestReview, submitPullRequest } from "@/lib/pull-request-run"
import type { IntentRun } from "@/lib/run-intent"

const { engine } = vi.hoisted(() => {
  const engine = {
    /** The intents that reached the engine, with the repository */
    asked: [] as { repositoryPath: string; intent: Intent }[],
    /** How the next run goes */
    run: null as IntentRun | null,
    /** The URLs of GitHub's form opened, and the Activity of the pull requests created */
    said: [] as string[],
    /** The run of the engine: records the intent, answers `run` */
    runIntent: (repositoryPath: string, intent: Intent): Promise<IntentRun | null> => {
      engine.asked.push({ repositoryPath, intent })
      return Promise.resolve(engine.run)
    },
  }
  return { engine }
})

vi.mock("@/lib/run-intent", async importOriginal => ({ ...(await importOriginal<typeof import("@/lib/run-intent")>()), runIntent: engine.runIntent }))
vi.mock("@/lib/git/new-pull-request", () => ({ openNewPullRequest: (url: string) => engine.said.push(`open ${url}`) }))
vi.mock("@/stores/activity", () => ({ recordPullRequestCreated: (title: string) => engine.said.push(`activity ${title}`) }))
vi.mock("@/stores/landed-stacks", () => ({ forgetLandedStackRoot: (root: string) => engine.said.push(`forget ${root}`) }))
vi.mock("@/stores/branch-sync", () => ({ setBranchPushing: () => {}, setBranchesPushProgress: () => {} }))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

const PULL_REQUEST = { number: 12, url: "https://github.com/acme/monorepo/pull/12", reviewers: [] } as unknown as PullRequest

/** The dialog of auth, opened in /other while the main window shows another repository */
const CONTEXT = {
  repositoryPath: "/other",
  branchPrefix: "user",
  branchName: "auth",
  label: "auth",
  wholeStack: null,
  head: "user/virtual/auth",
  base: "master",
  newPullRequestBlocker: null,
  repository: { owner: "acme", name: "monorepo" },
} as unknown as CreatePullRequestContext

const INPUT = { title: "Add auth", body: "b", reviewers: ["bob"], assignSelf: true, viewerLogin: "me" }

/** A run whose effects went as `effects` say, answering `result` */
function ran(effects: EffectRecord[], result: Outcome["result"]): IntentRun {
  const outcome = { op: "op", effects, newsKey: null, versionAfter: null, result, stats: {} }
  return { kind: "ran", plan: { effects: [] } as unknown as Plan, outcome: outcome as unknown as Outcome }
}

const DONE: EffectRecord = { effect: { kind: "createPullRequest" }, status: { kind: "done" } }

beforeEach(() => {
  engine.asked.length = 0
  engine.said.length = 0
})

describe("the creation of a pull request", () => {
  it("runs the intent in the repository of the dialog, and toasts and records the pull request GitHub opened", async () => {
    engine.run = ran([DONE], { kind: "pullRequest", pullRequest: PULL_REQUEST, created: true, assignees: ["me"], warnings: [] })
    const { toast, pullRequest } = await submitPullRequest(CONTEXT, INPUT)
    expect(engine.asked).toEqual([
      {
        repositoryPath: "/other",
        intent: { kind: "createPullRequest", subject: { kind: "branch", name: "auth" }, title: "Add auth", body: "b", assignSelf: true, reviewers: ["bob"], onGitHub: false },
      },
    ])
    expect([pullRequest.number, toast.title]).toEqual([12, "Created pull request #12"])
    expect(engine.said).toEqual(["activity Created pull request #12"])
  })

  it("records nothing of the one GitHub had of the head already, and throws why nothing was created", async () => {
    engine.run = ran([DONE], { kind: "pullRequest", pullRequest: PULL_REQUEST, created: false, assignees: [], warnings: [] })
    expect((await submitPullRequest(CONTEXT, INPUT)).pullRequest.number).toBe(12)
    expect(engine.said).toEqual([])
    const moved: ApiError = { code: "lease_rejected", message: "The remote branch of auth moved: nothing was pushed" } as ApiError
    const push: EffectRecord = { effect: { kind: "push", branches: ["auth"], force: false }, status: { kind: "failed", error: moved } }
    engine.run = ran([push, { ...DONE, status: { kind: "skipped", reason: null } }], { kind: "none" })
    await expect(submitPullRequest(CONTEXT, INPUT)).rejects.toThrow("The remote branch of auth moved: nothing was pushed")
  })

  it("opens GitHub's form once the run pushed and GitHub has the branch, of the whole stack too", async () => {
    engine.run = ran([], { kind: "none" })
    await continueOnGitHub({ ...CONTEXT, branchName: "air", wholeStack: { members: [] }, head: "user/stack/air" }, { title: "Air" })
    expect(engine.asked.map(asked => asked.intent)).toEqual([
      { kind: "createPullRequest", subject: { kind: "wholeStack", root: "air" }, title: "Air", body: "", assignSelf: false, reviewers: [], onGitHub: true },
    ])
    expect(engine.said).toEqual([expect.stringMatching(/^open https:\/\/github\.com\/acme\/monorepo\/compare\/master\.\.\.user\/stack\/air\?/)])
  })
})

describe("a review request", () => {
  it("pushes first and answers whom GitHub asked and refused, also when it asked nobody", async () => {
    const refused = { requested: [], refused: [{ login: "me", reason: "the author" }] }
    const failed = { code: "review_refused", message: "GitHub asked nobody" } as ApiError
    engine.run = ran([{ effect: { kind: "requestReviewers", number: 7, reviewers: ["me"] }, status: { kind: "failed", error: failed } }], { kind: "reviewers", ...refused })
    expect(await requestReview("stack:air", ["me"], "/other")).toEqual({ kind: "reviewers", ...refused })
    expect(engine.asked).toEqual([
      { repositoryPath: "/other", intent: { kind: "requestReview", subject: { kind: "wholeStack", root: "air" }, reviewers: ["me"], pushFirst: true } },
    ])
    engine.run = { kind: "noPlan" }
    await expect(requestReview("auth", ["me"], "/other")).rejects.toThrow("auth has no open pull request")
  })

  it("is the running operation of its row while the session runs it", () => {
    const running = { id: "op", intent: { kind: "requestReview", subject: { kind: "branch", name: "auth" }, reviewers: [], pushFirst: true } } as unknown as RunningOperation
    expect(pullRequestOperationIn({ op: running }, "requestReview", "auth")).toBe(running)
    expect(pullRequestOperationIn({ op: running }, "createPullRequest", "auth")).toBeUndefined()
    expect(pullRequestOperationIn({ op: running }, "requestReview", "login")).toBeUndefined()
  })
})
