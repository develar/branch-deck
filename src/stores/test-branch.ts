import type { Branch } from "@/stores/branch-sync"

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
