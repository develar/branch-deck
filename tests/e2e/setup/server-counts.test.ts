import { describe, expect, it } from "vite-plus/test"
import { describeServerCounts, holdsAnything } from "./server-counts"

describe("describeServerCounts", () => {
  it("says the server holds nothing of a run that left nothing, with the waits its run-out clocks served", () => {
    const counts = {
      repositories: 0,
      tests: [],
      deleting: [],
      keptContexts: [],
      runOutWaits: { total: 12, busiest: [{ repository: "repo-7", test: "patronus/patronus-actions.spec.ts:1617 › Patronus actions › should wait", waits: 12 }] },
      sessions: 0,
      orphanSessions: [],
      watches: { repositories: 0, sessions: 0, looks: 0 },
    }
    expect(holdsAnything(counts)).toBe(false)
    expect(describeServerCounts(counts)).toEqual([
      "The test server holds nothing of the run: 0 repositories, 0 deletions in progress, 0 kept apps, 0 sessions (0 orphans), 0 watched repositories of 0 sessions, 12 waits of run-out clocks",
      "  the run-out clock of repo-7 served 12 waits: patronus/patronus-actions.spec.ts:1617 › Patronus actions › should wait",
    ])
  })

  it("names what a run left behind by the test that made it: a repository, a deletion in progress, a kept app, an orphan with what keeps it", () => {
    const counts = {
      repositories: 1,
      tests: [{ repository: "repo-855", test: "repository/saved-path-validation.spec.ts:61 › Repository Validation › should clear error" }],
      deleting: [{ repository: "repo-900", test: "update-project/update-project.spec.ts:220 › Update Project › should update" }],
      keptContexts: [{ repository: "repo-560", test: "next-step/facts-without-answer.spec.ts:153 › Next step before the answers › should wait" }],
      sessions: 2,
      orphanSessions: [
        {
          worktree: "/tmp/repo-37",
          prefix: "user-name",
          repository: "repo-37",
          test: "amend-changes/amend-changes.spec.ts:80 › Amend › should submit",
          subscribers: 0,
          operations: 1,
        },
        { worktree: "/elsewhere", prefix: "user-name", repository: null, test: null, subscribers: 1, operations: 0 },
      ],
      watches: { repositories: 1, sessions: 1, looks: 4 },
    }
    expect(holdsAnything(counts)).toBe(true)
    expect(describeServerCounts(counts)).toEqual([
      "The test server holds what the run left: 1 repository, 1 deletion in progress, 1 kept app, 2 sessions (2 orphans), 1 watched repository of 1 session, 0 waits of run-out clocks",
      "  repository repo-855: repository/saved-path-validation.spec.ts:61 › Repository Validation › should clear error",
      "  deletion in progress of repo-900: update-project/update-project.spec.ts:220 › Update Project › should update",
      "  kept app of repo-560: next-step/facts-without-answer.spec.ts:153 › Next step before the answers › should wait",
      "  orphan session /tmp/repo-37 under user-name (0 windows, 1 operation): amend-changes/amend-changes.spec.ts:80 › Amend › should submit",
      "  orphan session /elsewhere under user-name (1 window, 0 operations): no test named",
    ])
  })

  it("waits for a deletion in progress: the server still holds it", () => {
    expect(holdsAnything({ repositories: 0, sessions: 0, deleting: [{ repository: "repo-1", test: "" }] })).toBe(true)
  })

  it("reads the counts of a server older than the names: the numbers alone", () => {
    const counts = { repositories: 0, sessions: 1 }
    expect(holdsAnything(counts)).toBe(true)
    expect(describeServerCounts(counts)).toEqual([
      "The test server holds what the run left: 0 repositories, 0 deletions in progress, 0 kept apps, 1 session (0 orphans), 0 watched repositories of 0 sessions, 0 waits of run-out clocks",
    ])
  })
})
