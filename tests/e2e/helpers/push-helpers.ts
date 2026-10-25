import type { Locator, Page, Request } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { TEST_SERVER_URL } from "./test-server-url"
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
  /** The unapplied branch `{prefix}/unapplied/<branchName>` (`push_branches` only) */
  unapplied?: boolean
}

/** The time of the newest commit of the templates: the page runs on a fixed clock */
export const PUSH_TIME = 1704139200

/** The outcome of `push_branch`: the remote status after the push */
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
      ancestorCommitsAhead: 0,
      remoteTip: result.remoteExists === false ? null : (result.remoteTip ?? `${branchName}-pushed`),
      myUnpushedCount: unpushedCommits.length,
      fastForward: result.fastForward ?? (commitsBehind === 0 && commitsAmended === 0 && commitsRestacked === 0),
      lastPushTime: PUSH_TIME,
    },
  }
}

/** The branches of the `stacked_branches` template */
export const STACKED_BRANCHES = ["auth", "login", "session", "oauth", "docs"]

/** The branches of the `stack_island` template: its stack from `air`, and `auth`, `login` and `docs` */
export const STACK_ISLAND_BRANCHES = ["air", "bazel", "bridge", "runtime", "side-fix", "deploy", "side-test", "auth", "login", "docs"]

type RemoteStatus = ReturnType<typeof pushOutcome>["remoteStatus"]

/**
 * What `push_branch` or `push_branches` answer for a push the page answered, `outcome`: the session of the repository
 * gets its remote statuses first, as the app's push gives them (`POST /sessions/pushed-in-page`, the server's own
 * `Sessions::push`; one that pushed lists the pull requests again, as the app's), and the answer names the version of
 * the view whose patch has them, which the page waits for as it waits after the app's push. `request` is the page's
 * request of the push, whose repository the server holds meanwhile.
 * A test may end while its push is still answered (it asserted what the push sent): its pages are closed and its
 * repository deleted, the server answers 410, and the answer goes to no page, so it names no version
 */
export async function answeredInSession<T extends { remoteStatus: RemoteStatus } | { remoteStatuses: RemoteStatus[] }>(
  params: { repositoryPath?: unknown },
  branches: readonly PushRequest[],
  outcome: T,
  request?: Request,
): Promise<{ outcome: T; version: unknown }> {
  const repositoryId = request?.headers()["x-repository-id"]
  const response = await fetch(`${TEST_SERVER_URL}/sessions/pushed-in-page`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...(repositoryId === undefined ? {} : { "X-Repository-Id": repositoryId }) },
    body: JSON.stringify({
      repositoryPath: params.repositoryPath,
      branches,
      remoteStatuses: "remoteStatus" in outcome ? [outcome.remoteStatus] : outcome.remoteStatuses,
      // a push that found the remote moved pushed nothing: the session lists no pull requests after it
      moved: (outcome as { type?: string }).type === "remoteMoved",
    }),
  })
  if (response.status === 410) {
    return { outcome, version: null }
  }
  if (!response.ok) {
    throw new Error(`The session did not take the push: ${response.status} ${await response.text()}`)
  }
  return { outcome, version: await response.json() }
}

/**
 * The remote statuses of `names` as a push of each left them (`pushOutcome`), for the remote statuses the next sync
 * reports (`SyncOverrides.remote`)
 */
export function pushedRemotes(names: readonly string[], result: PushResult = {}) {
  return Object.fromEntries(names.map(name => [name, pushOutcome(name, result).remoteStatus]))
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
 * (`push_branches`), with the remote status of each branch after it, which the session of the repository gets as from
 * the app's push (`answeredInSession`). A stack push whose result for a branch is
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
    const request = route.request().postDataJSON() as PushRequest & { repositoryPath: string }
    const result = next(request)
    await whilePushing(page, route.request().headers()["x-progress-channel"], [result])
    await route.fulfill({ json: await answeredInSession(request, [request], pushOutcome(request.branchName, result), route.request()) })
  })
  await page.route("**/invoke/push_branches", async route => {
    const request = route.request().postDataJSON() as { repositoryPath: string; branches: PushRequest[] }
    stackPushes.push(request.branches)
    const answered = request.branches.map(branch => ({ name: branch.branchName, wholeStack: branch.wholeStack === true, result: next(branch) }))
    await whilePushing(
      page,
      route.request().headers()["x-progress-channel"],
      answered.map(it => it.result),
    )
    const remoteStatuses = answered.map(it => pushOutcome(it.name, it.result).remoteStatus)
    const moved = answered.find(it => it.result?.outcome === "remoteMoved")
    const outcome = moved === undefined ? { type: "pushed", remoteStatuses } : { type: "remoteMoved", branchName: moved.name, wholeStack: moved.wholeStack, remoteStatuses }
    await route.fulfill({ json: await answeredInSession(request, request.branches, outcome, route.request()) })
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
