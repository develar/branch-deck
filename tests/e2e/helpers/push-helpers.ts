import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { sendProgress } from "./wait-helpers"

export interface DroppedCommit {
  id: string
  subject: string
  author: string
  authorTime: number
}

export interface PushResult {
  /** `remoteMoved`: the remote branch was not where the row showed it, nothing was pushed; the status is the fetched one */
  outcome?: "pushed" | "remoteMoved"
  /** Defaults to a remote branch; false for a branch of a refused stack push that is still not on the remote */
  remoteExists?: boolean
  unpushedCommits?: string[]
  commitsBehind?: number
  /** Older versions of commits amended here on the remote */
  commitsAmended?: number
  /** Older versions of the commits of the branches it is built on on the remote */
  commitsRestacked?: number
  /** Defaults to no remote commit the branch lacks, none an older version either */
  fastForward?: boolean
  remoteTip?: string
  droppedCommits?: DroppedCommit[]
  /** Sent to the progress channel of the push before it ends, as the backend reports how far git is */
  progress?: PushProgress[]
  /** The push ends once this resolves, so a test can see the running push */
  release?: Promise<void>
}

/** A `TransferProgress` of the bindings */
export interface PushProgress {
  percent: number
  transferred: string | null
  speed: string | null
  line: string
}

/** The parameters of a `push_branch` call, and of each branch of a `push_branches` call */
export interface PushRequest {
  branchName: string
  parentBranch?: string | null
  expectedRemote: string | null
  /** Push Up to Here: the commit pushed instead of the tip */
  tip?: string | null
  /** The stack branch of the whole stack whose root `branchName` is (`push_branches` only) */
  wholeStack?: boolean
}

/** The time of the newest commit of the templates: the page runs on a fixed clock */
export const PUSH_TIME = 1704139200

/** What `push_branch` answers: the outcome with the remote status after the push */
export function pushOutcome(branchName: string, result: PushResult = {}) {
  const unpushedCommits = result.unpushedCommits ?? []
  const commitsBehind = result.commitsBehind ?? 0
  const commitsAmended = result.commitsAmended ?? 0
  const commitsRestacked = result.commitsRestacked ?? 0
  return {
    type: result.outcome ?? "pushed",
    remoteStatus: {
      branchName,
      remoteExists: result.remoteExists ?? true,
      unpushedCommits,
      commitsBehind,
      droppedCommits: result.droppedCommits ?? [],
      commitsAmended,
      commitsRestacked,
      commitsLanded: 0,
      landedCommits: [],
      commitsReworded: 0,
      remoteTip: result.remoteExists === false ? null : (result.remoteTip ?? `${branchName}-pushed`),
      myUnpushedCount: unpushedCommits.length,
      fastForward: result.fastForward ?? (commitsBehind === 0 && commitsAmended === 0 && commitsRestacked === 0),
      lastPushTime: PUSH_TIME,
    },
  }
}

/**
 * What each branch of a push answers, by branch (the stack branch of the whole stack of a root by `stack:<root>`,
 * `wholeStackResultKey`): a list answers the pushes of a branch in turn, its last result repeating
 */
export type PushResults = Record<string, PushResult | PushResult[]>

/** The key of the results of the stack branch of the whole stack of `root` (`PushRequest.wholeStack`), apart from its root's */
export function wholeStackResultKey(root: string): string {
  return `stack:${root}`
}

function resultKey(request: PushRequest): string {
  return request.wholeStack === true ? wholeStackResultKey(request.branchName) : request.branchName
}

/**
 * Answers the pushes in the page (the E2E server has no remote), `push_branch` and the one atomic push of a stack
 * (`push_branches`), with the remote status of each branch after it. A stack push whose result for a branch is
 * `remoteMoved` refuses all of its branches: the outcome names that branch, and every branch answers its result as the
 * status fetched after the refusal (give the others their `unpushedCommits`: nothing of theirs was pushed); the stack
 * branch of a whole stack answers its results by `wholeStackResultKey`. `branches`
 * lists every branch pushed, in order, a stack push one entry per branch; `stackPushes` the stack pushes, each with its
 * branches, parents first.
 */
async function routePushes(page: Page, results: PushResults): Promise<{ branches: PushRequest[]; stackPushes: PushRequest[][] }> {
  const branches: PushRequest[] = []
  const stackPushes: PushRequest[][] = []
  const next = (request: PushRequest): PushResult | undefined => {
    const branchResults = results[resultKey(request)] ?? {}
    const list = Array.isArray(branchResults) ? branchResults : [branchResults]
    const pushes = branches.filter(previous => resultKey(previous) === resultKey(request)).length
    branches.push(request)
    return list[Math.min(pushes, list.length - 1)]
  }
  await page.route("**/invoke/push_branch", async route => {
    const request = route.request().postDataJSON() as PushRequest
    const result = next(request)
    await whilePushing(page, route.request().headers()["x-progress-channel"], [result])
    await route.fulfill({ json: pushOutcome(request.branchName, result) })
  })
  await page.route("**/invoke/push_branches", async route => {
    const request = route.request().postDataJSON() as { branches: PushRequest[] }
    stackPushes.push(request.branches)
    const answered = request.branches.map(branch => ({ name: branch.branchName, wholeStack: branch.wholeStack === true, result: next(branch) }))
    await whilePushing(
      page,
      route.request().headers()["x-progress-channel"],
      answered.map(it => it.result),
    )
    const remoteStatuses = answered.map(it => pushOutcome(it.name, it.result).remoteStatus)
    const moved = answered.find(it => it.result?.outcome === "remoteMoved")
    await route.fulfill({
      json: moved === undefined ? { type: "pushed", remoteStatuses } : { type: "remoteMoved", branchName: moved.name, wholeStack: moved.wholeStack, remoteStatuses },
    })
  })
  return { branches, stackPushes }
}

/** Sends the progress of the results to the channel of the push, as the backend reports how far git is, then waits for their release */
async function whilePushing(page: Page, channel: string | undefined, results: readonly (PushResult | undefined)[]): Promise<void> {
  for (const result of results) {
    if (result?.progress !== undefined && channel !== undefined) {
      await sendProgress(page, channel, result.progress)
    }
  }
  await Promise.all(results.flatMap(result => (result?.release === undefined ? [] : [result.release])))
}

/**
 * Answers the pushes in the page (`routePushes`): a push of one branch and the one push of a stack. Returns the branches
 * pushed, in order, a stack push one entry per branch.
 */
export async function routePush(page: Page, results: PushResults): Promise<PushRequest[]> {
  return (await routePushes(page, results)).branches
}

/**
 * Answers the pushes in the page as `routePush` does, for a test of the one atomic push of a stack: `stackPushes` are
 * the stack pushes (`push_branches`), each with its branches, parents first; `pushes` every branch pushed, by either
 * command, in order
 */
export async function routePushBranches(page: Page, results: PushResults = {}): Promise<{ stackPushes: PushRequest[][]; pushes: PushRequest[] }> {
  const { branches, stackPushes } = await routePushes(page, results)
  return { stackPushes, pushes: branches }
}

/**
 * Routes the pushes as `routePush` does, the push of `branchName` held at `percent` until `release`: what shows while a
 * push runs; a stack push with it is held as a whole. `pushes` lists the branches pushed, `stackPushes` the stack pushes.
 */
export async function routeHeldPush(page: Page, branchName: string, percent: number): Promise<{ pushes: PushRequest[]; stackPushes: PushRequest[][]; release: () => void }> {
  let release!: () => void
  const held = new Promise<void>(resolve => (release = resolve))
  const { branches, stackPushes } = await routePushes(page, {
    [branchName]: { progress: [{ percent, transferred: null, speed: null, line: `Writing objects:  ${percent}% (2/4)` }], release: held },
  })
  return { pushes: branches, stackPushes, release }
}

/**
 * A push that drops remote commits asks first: the click on its button (the toast's action, P in the preview) opens its
 * description with the dropped commits and a destructive Force Push button; this clicks that button, which pushes.
 */
export async function confirmForcePush(page: Page, label: string): Promise<void> {
  // the description of another push may still be closing
  const description = page.getByTestId("push-description").filter({ has: page.getByTestId("push-confirm") })
  await expect(description.getByTestId("dropped-commits")).toBeVisible()
  const confirm = description.getByTestId("push-confirm")
  await expect(confirm).toHaveText(label)
  await confirm.click()
}

/**
 * The toast of a push of `branchName`, by its title: "Pushed feature-auth", or "Force pushed feature-auth" (the words
 * of its Activity entry)
 */
export function pushedToast(page: Page, branchName: string): Locator {
  const name = branchName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  return page.locator("[data-slot=toast]").filter({ has: page.locator("[data-slot=toast-title]", { hasText: new RegExp(`^(Force p|P)ushed ${name}$`) }) })
}

/** Waits for the toast of a push of `branchName`; with `description`, its detail ("3 commits · created the remote branch") */
export async function expectPushedToast(page: Page, branchName: string, description?: string): Promise<void> {
  const toast = pushedToast(page, branchName)
  await expect(toast).toBeVisible()
  if (description !== undefined) {
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(description)
  }
}
