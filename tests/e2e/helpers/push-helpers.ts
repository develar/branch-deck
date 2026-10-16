import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"

export interface DroppedCommit {
  id: string
  subject: string
  author: string
  authorTime: number
}

export interface PushResult {
  /** `remoteMoved`: the remote branch was not where the row showed it, nothing was pushed; the status is the fetched one */
  outcome?: "pushed" | "remoteMoved"
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

/** The parameters of a `push_branch` call */
export interface PushRequest {
  branchName: string
  parentBranch?: string | null
  expectedRemote: string | null
  /** Push Up to Here: the commit pushed instead of the tip */
  tip?: string | null
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
      remoteExists: true,
      unpushedCommits,
      commitsBehind,
      droppedCommits: result.droppedCommits ?? [],
      commitsAmended,
      commitsRestacked,
      remoteTip: result.remoteTip ?? `${branchName}-pushed`,
      myUnpushedCount: unpushedCommits.length,
      fastForward: result.fastForward ?? (commitsBehind === 0 && commitsAmended === 0 && commitsRestacked === 0),
      lastPushTime: PUSH_TIME,
    },
  }
}

/**
 * Answers `push_branch` in the page (the E2E server has no remote) with the remote status after the push. A list
 * answers the pushes of a branch in turn, its last result repeating. Returns the requests, in order.
 */
export async function routePush(page: Page, results: Record<string, PushResult | PushResult[]>): Promise<PushRequest[]> {
  const requests: PushRequest[] = []
  await page.route("**/invoke/push_branch", async (route) => {
    const request = route.request().postDataJSON() as PushRequest
    const branchResults = results[request.branchName] ?? {}
    const list = Array.isArray(branchResults) ? branchResults : [branchResults]
    const pushes = requests.filter(previous => previous.branchName === request.branchName).length
    requests.push(request)
    const result = list[Math.min(pushes, list.length - 1)]
    const channel = route.request().headers()["x-progress-channel"]
    if (result?.progress !== undefined && channel !== undefined) {
      await page.evaluate(({ channel, messages }) => {
        // registered by the mock of the page (`registerProgressChannel` in tauri-mocks.ts)
        const channels = (window as unknown as { __progressChannels: Record<string, { onmessage: (message: PushProgress) => void }> }).__progressChannels
        for (const message of messages) {
          channels[channel]!.onmessage(message)
        }
      }, { channel, messages: result.progress })
    }
    await result?.release
    await route.fulfill({ json: pushOutcome(request.branchName, result) })
  })
  return requests
}

/**
 * Routes the pushes as `routePush` does, the push of `branchName` held at `percent` until `release`: what shows while a
 * push runs
 */
export async function routeHeldPush(page: Page, branchName: string, percent: number): Promise<{ pushes: PushRequest[], release: () => void }> {
  let release!: () => void
  const held = new Promise<void>(resolve => release = resolve)
  const pushes = await routePush(page, {
    [branchName]: { progress: [{ percent, transferred: null, speed: null, line: `Writing objects:  ${percent}% (2/4)` }], release: held },
  })
  return { pushes, release }
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
