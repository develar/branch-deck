import type { Page } from "@playwright/test"

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
  /** Defaults to no remote commit the branch lacks */
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
}

/** The time of the newest commit of the templates: the page runs on a fixed clock */
export const PUSH_TIME = 1704139200

/** What `push_branch` answers: the outcome with the remote status after the push */
export function pushOutcome(branchName: string, result: PushResult = {}) {
  const unpushedCommits = result.unpushedCommits ?? []
  const commitsBehind = result.commitsBehind ?? 0
  return {
    type: result.outcome ?? "pushed",
    remoteStatus: {
      branchName,
      remoteExists: true,
      unpushedCommits,
      commitsBehind,
      droppedCommits: result.droppedCommits ?? [],
      remoteTip: result.remoteTip ?? `${branchName}-pushed`,
      myUnpushedCount: unpushedCommits.length,
      fastForward: result.fastForward ?? commitsBehind === 0,
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
