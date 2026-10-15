import type { AppliedUpdate, LandedBranch, UpdateCheck } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { joinNames } from "@/lib/text"

/*
 * The landed news has one grammar wherever it shows (the Update tooltip, the footer of the outgoing overview, the toast,
 * the island, the chip): each number has its verb, "3 commits of IJPL-257337 landed · 9 stay local", never a ratio.
 */

/** The name of a landed branch; commits without a prefix have none */
export function landedBranchName(branch: LandedBranch): string {
  return branch.name ?? "Unassigned"
}

/** "1 stays local", "12 stay local": the commits of a branch the baseline doesn't have */
export function formatStayLocal(count: number): string {
  return count === 1 ? "1 stays local" : `${count} stay local`
}

/**
 * What of one branch landed and what stays, for a row of the island: "40 landed · 12 stay local", "1 landed",
 * "3 landed · squashed". Compact, for a chip of the branch row: "40 landed · 12 local".
 */
export function landedCountLabel(branch: LandedBranch, compact = false): string {
  const parts = [`${branch.commits.length} landed`]
  if (!compact && isSquashedUpstream(branch)) {
    parts.push("squashed")
  }
  if (branch.remaining > 0) {
    parts.push(compact ? `${branch.remaining} local` : formatStayLocal(branch.remaining))
  }
  return parts.join(" · ")
}

/**
 * The fetched baseline has every commit of the branch and the update is still to come (`incoming`): the update archives
 * the branch, there is nothing to push. After the update a branch still in the table has new commits.
 */
export function isFullyLanded(branch: LandedBranch, incoming: boolean): boolean {
  return incoming && branch.remaining === 0
}

/** The names of the branches of the landing (`BaselineStatus.landing`) that the fetched baseline has all of */
export function fullyLandedNames(landing: readonly LandedBranch[]): ReadonlySet<string> {
  return new Set(landing.flatMap(branch => branch.name !== null && isFullyLanded(branch, true) ? [branch.name] : []))
}

/** The description of the status and of the push button of a fully landed branch */
export function landedBranchDescription(upstream: string): string {
  return `Already in ${upstream}. Update to archive the branch.`
}

/**
 * The chip of an active branch: "Landed" when the fetched baseline has all of it, else "40 landed · 12 local". After
 * the update a fully landed branch moves to the archive; one still in the table has new commits, so it is "3 landed".
 */
export function landedChipLabel(branch: LandedBranch, incoming: boolean): string {
  return isFullyLanded(branch, incoming) ? "Landed" : landedCountLabel(branch, true)
}

/** Every commit of the branch landed in another form: the baseline has its change, but no commit with its subject */
export function isSquashedUpstream(branch: LandedBranch): boolean {
  return branch.commits.every(commit => commit.upstream === null)
}

/** The committer of the baseline commits of a branch (the robot or the person who merged it); none when squashed */
export function landedCommitter(branch: LandedBranch): string | undefined {
  return branch.commits.find(commit => commit.upstream !== null)?.upstream?.committer
}

/** The committer when every branch of the report has the same one: the island names it once, not on every row */
export function sharedCommitter(report: readonly LandedBranch[]): string | undefined {
  const committers = report.map(landedCommitter)
  const first = committers[0]
  return first !== undefined && committers.every(committer => committer === first) ? first : undefined
}

function yourCommits(count: number): string {
  return `${count} of your commits`
}

/** What of one branch landed, as the subject of "landed": "wsm", "3 commits of IJPL-257337", "2 of your commits" */
function landedSubject(branch: LandedBranch): string {
  const landed = branch.commits.length
  if (branch.name === null) {
    return yourCommits(landed)
  }
  return branch.remaining === 0 ? branch.name : `${formatCommitCount(landed)} of ${branch.name}`
}

/** One branch in the list after "your": "wsm", "3 commits of IJPL-257337", "2 commits without a branch" */
function landingPhrase(branch: LandedBranch): string {
  return branch.name === null ? `${formatCommitCount(branch.commits.length)} without a branch` : landedSubject(branch)
}

/**
 * What the fetched baseline already has of the user's work, for the tooltip of the Update button: "brings in your wsm
 * and 3 commits of IJPL-257337"; null when nothing landed. Names the user's own branches, never a count of upstream
 * commits.
 */
export function describeLanding(landing: readonly LandedBranch[]): string | null {
  return landing.length === 0 ? null : `brings in your ${joinNames(landing.map(landingPhrase))}`
}

/**
 * The landing in a few words, for the footer of the outgoing overview: "your wsm landed", "40 of your commits landed",
 * "your 2 branches landed"
 */
export function shortLanding(landing: readonly LandedBranch[]): string | null {
  const branch = landing[0]
  if (branch === undefined) {
    return null
  }
  if (landing.length > 1) {
    return `your ${landing.length} branches landed`
  }
  return branch.name === null || branch.remaining > 0 ? `${yourCommits(branch.commits.length)} landed` : `your ${branch.name} landed`
}

export interface UpdateToast {
  title: string
  description?: string
}

/** "rebased 116 commits" or "fast-forwarded" */
function appliedWords(applied: AppliedUpdate): string {
  return applied.kind === "fastForward" ? "fast-forwarded" : `rebased ${formatCommitCount(applied.replayed)}`
}

/**
 * What landed, as the news: "wsm landed in origin/master", "40 commits of IJPL-257337 landed · 12 stay local"; several
 * branches are "3 branches landed in origin/master" with their names as the description. Null when nothing landed.
 */
export function landedNews(landed: readonly LandedBranch[], upstream: string): UpdateToast | null {
  const only = landed[0]
  if (only === undefined) {
    return null
  }
  if (landed.length > 1) {
    return { title: `${landed.length} branches landed in ${upstream}`, description: joinNames(landed.map(landedBranchName)) }
  }
  return {
    title: only.remaining > 0
      ? `${landedSubject(only)} landed · ${formatStayLocal(only.remaining)}`
      : `${landedSubject(only)} landed in ${upstream}${isSquashedUpstream(only) ? ", squashed" : ""}`,
  }
}

/**
 * The toast after an applied update. What landed is the news and leads; how the branch was updated follows. Without
 * anything landed, the title says how the branch was updated, as before.
 */
export function appliedUpdateToast(check: UpdateCheck, applied: AppliedUpdate, landed: readonly LandedBranch[]): UpdateToast {
  const { branch, upstream } = check
  const news = landedNews(landed, upstream)
  if (news === null) {
    return {
      title: applied.kind === "fastForward"
        ? `Updated ${branch}: fast-forwarded to ${upstream}`
        : `Updated ${branch}: rebased ${formatCommitCount(applied.replayed)} onto ${upstream}`,
    }
  }
  return {
    title: news.title,
    description: news.description === undefined ? `Updated ${branch}: ${appliedWords(applied)}` : `${news.description} · ${appliedWords(applied)}`,
  }
}

/** The report of a new update merged into the undismissed one, as the backend merges it: by branch name, new commits added */
export function mergeLandedReport(report: readonly LandedBranch[], found: readonly LandedBranch[]): LandedBranch[] {
  const merged = report.map(branch => ({ ...branch, commits: [...branch.commits] }))
  for (const branch of found) {
    const known = merged.find(it => it.name === branch.name)
    if (known === undefined) {
      merged.push(branch)
      continue
    }
    const ids = new Set(known.commits.map(commit => commit.id))
    known.commits.push(...branch.commits.filter(commit => !ids.has(commit.id)))
    known.remaining = branch.remaining
    known.landedAt = Math.max(known.landedAt, branch.landedAt)
  }
  return merged
}
