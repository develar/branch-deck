import type { AppliedUpdate, LandedBranch, UpdateCheck } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"

/** At most this many branch names in a sentence */
const NAMED_BRANCHES = 3

/** The name of a landed branch; commits without a prefix have none */
export function landedBranchName(branch: LandedBranch): string {
  return branch.name ?? "Unassigned"
}

/** "5 commits" for a whole branch, "3 of 12" for part of one */
export function landedCountLabel(branch: LandedBranch): string {
  const landed = branch.commits.length
  return branch.remaining === 0 ? formatCommitCount(landed) : `${landed} of ${landed + branch.remaining}`
}

/** Every commit of the branch landed in another form: the baseline has its change, but no commit with its subject */
export function isSquashedUpstream(branch: LandedBranch): boolean {
  return branch.commits.every(commit => commit.upstream === null)
}

/** "a", "a and b", "a, b and c", "a, b, c and 2 more" */
function joinNames(names: readonly string[]): string {
  if (names.length <= 1) {
    return names.join("")
  }
  if (names.length > NAMED_BRANCHES) {
    return `${names.slice(0, NAMED_BRANCHES).join(", ")} and ${names.length - NAMED_BRANCHES} more`
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`
}

/** One branch as a phrase after "your": "wsm", "3 of 12 commits of IJPL-257337", "2 commits without a branch" */
function landingPhrase(branch: LandedBranch): string {
  const landed = branch.commits.length
  if (branch.name === null) {
    return `${formatCommitCount(landed)} without a branch`
  }
  return branch.remaining === 0 ? branch.name : `${landed} of ${landed + branch.remaining} commits of ${branch.name}`
}

/**
 * What the fetched baseline already has of the user's work, for the tooltip of the Update button: "brings in your wsm
 * and 3 of 12 commits of IJPL-257337"; null when nothing landed. Names the user's own branches, never a count of
 * upstream commits.
 */
export function describeLanding(landing: readonly LandedBranch[]): string | null {
  return landing.length === 0 ? null : `brings in your ${joinNames(landing.map(landingPhrase))}`
}

/** The landing in a few words, for the footer of the outgoing overview: "your wsm landed", "your 2 branches landed" */
export function shortLanding(landing: readonly LandedBranch[]): string | null {
  const branch = landing[0]
  if (branch === undefined) {
    return null
  }
  if (landing.length > 1) {
    return `your ${landing.length} branches landed`
  }
  return branch.name === null || branch.remaining > 0 ? `${formatCommitCount(branch.commits.length)} of yours landed` : `your ${branch.name} landed`
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
 * The toast after an applied update. What landed is the news and leads; how the branch was updated follows. Without
 * anything landed, the title says how the branch was updated, as before.
 */
export function appliedUpdateToast(check: UpdateCheck, applied: AppliedUpdate, landed: readonly LandedBranch[]): UpdateToast {
  const { branch, upstream } = check
  const only = landed[0]
  if (only === undefined) {
    return {
      title: applied.kind === "fastForward"
        ? `Updated ${branch}: fast-forwarded to ${upstream}`
        : `Updated ${branch}: rebased ${formatCommitCount(applied.replayed)} onto ${upstream}`,
    }
  }
  if (landed.length > 1) {
    return {
      title: `${landed.length} branches landed in ${upstream}`,
      description: `${joinNames(landed.map(landedBranchName))} · ${appliedWords(applied)}`,
    }
  }

  const count = only.commits.length
  if (only.name === null) {
    return { title: `${formatCommitCount(count)} of yours landed in ${upstream}`, description: `Updated ${branch}: ${appliedWords(applied)}` }
  }
  if (only.remaining > 0) {
    return {
      title: `${count} of ${count + only.remaining} commits of ${only.name} landed`,
      description: `${only.remaining} stay local · ${appliedWords(applied)}`,
    }
  }
  return {
    title: `${only.name} landed in ${upstream}${isSquashedUpstream(only) ? ", squashed" : ""}`,
    description: `Updated ${branch}: ${appliedWords(applied)}`,
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
