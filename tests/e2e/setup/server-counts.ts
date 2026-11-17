/**
 * What the test server holds (`GET /debug/counts`, `debug_counts` of crates/test-server/src/lib.rs), as the global
 * teardown prints it at the end of a run: what the run left behind, each by the test that made it
 */
export interface ServerCounts {
  repositories: number
  /** The test of each repository still there */
  tests?: { repository: string; test: string }[]
  /** The deleted repositories whose deletion still waits for what runs for them: not left behind yet */
  deleting?: { repository: string; test: string }[]
  /** The deleted repositories whose app something they started still holds once their deletion ended */
  keptContexts?: { repository: string; test: string }[]
  /** The waits the clocks of deleted repositories served once they ran out, and the clocks that served the most */
  runOutWaits?: { total: number; busiest: { repository: string | null; test: string | null; waits: number }[] }
  sessions: number
  /** The sessions of a worktree no live test repository has, with what keeps each */
  orphanSessions?: { worktree: string; prefix: string; repository: string | null; test: string | null; subscribers: number; operations: number }[]
  watches?: { repositories: number; sessions: number; looks: number }
}

/**
 * Whether the server holds anything of a run: a repository, a deletion in progress, the app of a deleted repository, a
 * session, a watch. The waits of the clocks that ran out are a count of the whole run, nothing held
 */
export function holdsAnything(counts: ServerCounts): boolean {
  return counts.repositories > 0 || (counts.deleting?.length ?? 0) > 0 || (counts.keptContexts?.length ?? 0) > 0 || counts.sessions > 0 || (counts.watches?.repositories ?? 0) > 0
}

const plural = (count: number, one: string, many = `${one}s`) => `${count} ${count === 1 ? one : many}`

/**
 * The lines of the teardown: one of the counts, then one per thing left behind, each with its test, and the clocks of
 * deleted repositories that served the most waits once they ran out (a loop without a deadline serves ever more)
 */
export function describeServerCounts(counts: ServerCounts): string[] {
  const orphans = counts.orphanSessions ?? []
  const deleting = counts.deleting ?? []
  const kept = counts.keptContexts ?? []
  const watches = counts.watches ?? { repositories: 0, sessions: 0, looks: 0 }
  const runOut = counts.runOutWaits ?? { total: 0, busiest: [] }
  const summary = [
    plural(counts.repositories, "repository", "repositories"),
    plural(deleting.length, "deletion in progress", "deletions in progress"),
    plural(kept.length, "kept app"),
    `${plural(counts.sessions, "session")} (${plural(orphans.length, "orphan")})`,
    `${plural(watches.repositories, "watched repository", "watched repositories")} of ${plural(watches.sessions, "session")}`,
    plural(runOut.total, "wait of a run-out clock", "waits of run-out clocks"),
  ].join(", ")
  const tested = (test: string | null | undefined) => test || "no test named"
  return [
    holdsAnything(counts) ? `The test server holds what the run left: ${summary}` : `The test server holds nothing of the run: ${summary}`,
    ...(counts.tests ?? []).map(it => `  repository ${it.repository}: ${tested(it.test)}`),
    ...deleting.map(it => `  deletion in progress of ${it.repository}: ${tested(it.test)}`),
    ...kept.map(it => `  kept app of ${it.repository}: ${tested(it.test)}`),
    ...orphans.map(it => `  orphan session ${it.worktree} under ${it.prefix} (${plural(it.subscribers, "window")}, ${plural(it.operations, "operation")}): ${tested(it.test)}`),
    ...runOut.busiest.map(it => `  the run-out clock of ${it.repository ?? "no repository"} served ${plural(it.waits, "wait")}: ${tested(it.test)}`),
  ]
}
