import type { Page } from "@playwright/test"

export interface PushResult {
  unpushedCommits?: string[]
  commitsBehind?: number
  /** Defaults to no remote commit the branch lacks */
  fastForward?: boolean
}

/** Answers `push_branch` in the page (the E2E server has no remote): the remote status after the push. */
export async function routePush(page: Page, results: Record<string, PushResult>) {
  await page.route("**/invoke/push_branch", async (route) => {
    const branchName = (route.request().postDataJSON() as { branchName: string }).branchName
    const result = results[branchName] ?? {}
    const unpushedCommits = result.unpushedCommits ?? []
    await route.fulfill({
      json: {
        branchName,
        remoteExists: true,
        unpushedCommits,
        commitsBehind: result.commitsBehind ?? 0,
        myUnpushedCount: unpushedCommits.length,
        fastForward: result.fastForward ?? (result.commitsBehind ?? 0) === 0,
        lastPushTime: 1704139200,
      },
    })
  })
}
