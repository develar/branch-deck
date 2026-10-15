import type { Branch, RemoteStatus } from "@/stores/branch-sync"

/** The remote status of a pushed branch that is up to date; `overrides` replace any field */
export function testRemoteStatus(overrides: Partial<RemoteStatus> = {}): RemoteStatus {
  return {
    exists: true,
    unpushedCommits: [],
    commitsAhead: 0,
    commitsBehind: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 0,
    remoteTip: null,
    droppedCommits: [],
    ...overrides,
  }
}

/** A synced branch of one commit for tests, stacked on `parent` (null: a root); `overrides` replace any field */
export function testBranch(name: string, parent: string | null, overrides: Partial<Branch> = {}): Branch {
  return {
    name,
    commits: [],
    commitIndex: {},
    commitCount: 1,
    status: "Unchanged",
    statusText: "",
    processedCount: 0,
    hasError: false,
    autoExpandRequested: false,
    autoScrollRequested: false,
    latestCommitTime: 0,
    summary: "",
    descriptionTitle: null,
    remoteStatus: null,
    allCommitsHaveIssueReferences: false,
    myEmail: null,
    origin: "Prefix",
    issueKey: null,
    stack: {
      declaredParent: parent,
      parentState: parent === null ? null : "Active",
      effectiveBase: parent,
      blockedBy: null,
      problems: [],
      prBaseChanged: false,
      childPrecedesParent: false,
    },
    ...overrides,
  }
}
