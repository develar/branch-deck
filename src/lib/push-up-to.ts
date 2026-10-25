import { formatCommitCount } from "@/lib/commit"
import type { ParkedState } from "@/lib/parked"
import { formatParkedBlocker } from "@/lib/parked"
import type { LandedBy, PushHold } from "@/lib/patronus"
import { formatLandingSubject, isLandedCommit } from "@/lib/patronus"
import { formatForcePushPart } from "@/lib/remote-status"
import { stackAncestors } from "@/lib/stack"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"

/** One push of a Push Up to Here, root first */
export interface PushUpToStep {
  readonly name: string
  /** The commit to push instead of the tip (`StackPushBranch.tip`); undefined pushes the whole branch */
  readonly tip?: string
  /** The commits the push sends, of `total` the branch has for the remote */
  readonly sent: number
  readonly total: number
}

/** Push Up to Here can't push the commit, and why */
export interface PushUpToRefusal {
  readonly ok: false
  readonly reason: string
  /** The branch that stands in the way, which the reason names */
  readonly show?: string
}

/** What Push Up to Here of a commit pushes, or why it can't */
export type PushUpToPlan = { readonly ok: true; readonly steps: readonly PushUpToStep[] } | PushUpToRefusal

export interface PushUpToSources {
  readonly branchOf: (name: string) => Branch | undefined
  /**
   * What holds the push of `name` (`PushHold`): its own running Safe Push, a running one of a branch built on it or of
   * the whole stack that lands it whole, or a merged one that landed it; null while none does
   */
  readonly holdOf: (name: string) => PushHold | null
  /** The merged Safe Push that landed `name` (`getLandedBy`): its landed commits offer nothing; null for none */
  readonly landedOf: (name: string) => LandedBy | null
  /** Why `name` stays local (`getParkedState`): parked, or built on a parked branch; null while it is not */
  readonly parkedOf: (name: string) => ParkedState | null
}

/**
 * Whether a commit of the branch offers Push Up to Here, by the push arrow after its subject and the item of its menu:
 * the remote lacks it (`unpushedCommits`; nothing is known before the remote status loads), and no merged Safe Push
 * landed it (`isLandedCommit`), which puts it in the target as a commit the remote has
 */
export function offersPushUpTo(branch: Branch, commit: SyncedCommit, landed: LandedBy | null): boolean {
  const remote = branch.remoteStatus
  return remote != null && commit.hash !== undefined && remote.unpushedCommits.includes(commit.hash) && !isLandedCommit(landed, commit.originalHash)
}

/**
 * Push Up to Here of the commit `originalHash` of the branch `name`: the branch up to the commit, after every branch it
 * is built on that has work for the remote, whole (the branch is built on their tips), root first. Only a commit the
 * remote lacks on a branch whose remote is an older state of it (not on the remote, or a fast-forward) can be a cut: a
 * part of a branch rewritten since its push would rewind its pull request. An ancestor that can't be pushed (blocked,
 * failed to sync) or asks first (its push drops remote commits), and a branch a running Safe Push lands, stop it; an
 * ancestor a merged Safe Push landed whole is left out, as the push of the branch leaves it; a parked branch, and one
 * built on it, stays local (`formatParkedBlocker`). Null for a commit that offers nothing (`offersPushUpTo`: the remote
 * has it, or a merged Safe Push landed it), on every surface (the arrow and the menu of the full list).
 */
export function pushUpToPlan(name: string, originalHash: string, { branchOf, holdOf, landedOf, parkedOf }: PushUpToSources): PushUpToPlan | null {
  const branch = branchOf(name)
  const commit = branch?.commits.find(it => it.originalHash === originalHash)
  const remote = branch?.remoteStatus
  if (branch === undefined || commit?.hash === undefined || remote == null || !offersPushUpTo(branch, commit, landedOf(name))) {
    return null
  }
  // in the words of its push button
  const parked = parkedOf(name)
  if (parked !== null) {
    const reason = formatParkedBlocker(parked)
    return parked.with === null ? { ok: false, reason } : { ok: false, reason, show: parked.with }
  }
  const at = remote.unpushedCommits.indexOf(commit.hash)
  const own = blocker(branch, holdOf)
  if (own !== null) {
    return own
  }
  if (remote.exists && !remote.fastForward) {
    return { ok: false, reason: `${name} was rewritten since its push: it pushes whole.` }
  }
  const steps: PushUpToStep[] = []
  for (const ancestor of stackAncestors(name, branchOf)) {
    // all of it is in the target: the branch is pushed without it
    if (holdOf(ancestor.name)?.kind === "landed") {
      continue
    }
    const stop = blocker(ancestor, holdOf)
    if (stop !== null) {
      return stop
    }
    // the blocker above refused an ancestor the sync did not build: Rust's action of its remote status decides
    const action = ancestor.remoteStatus?.action
    if (ancestor.remoteStatus != null && action?.kind === "force" && action.drops) {
      const behind = ancestor.remoteStatus.commitsBehind
      return {
        ok: false,
        reason: `${ancestor.name}'s push drops ${behind === 1 ? "1 remote commit" : `${behind} remote commits`}: Force Push ${ancestor.name} first.`,
        show: ancestor.name,
      }
    }
    if (action !== undefined && action.kind !== "upToDate") {
      const total = ancestor.remoteStatus?.unpushedCommits.length ?? 0
      steps.push({ name: ancestor.name, sent: total, total })
    }
  }
  const total = remote.unpushedCommits.length
  // the last commit the remote lacks is the tip: the branch's own push
  steps.push(at === total - 1 ? { name, sent: total, total } : { name, tip: commit.hash, sent: at + 1, total })
  return { ok: true, steps }
}

/**
 * Why the branch can't be pushed in a Push Up to Here: blocked, failed to sync, its own Safe Push running, or landed
 * whole by the running Safe Push of a branch built on it or of the whole stack. A merged one stops nothing: its commits
 * offer nothing, and a branch it landed whole is left out.
 */
function blocker(branch: Branch, holdOf: (name: string) => PushHold | null): PushUpToRefusal | null {
  if (branch.status === "Blocked" || branch.hasError) {
    return { ok: false, reason: `${branch.name} can't be pushed: ${branch.statusText === "" ? "it failed to sync" : branch.statusText}.`, show: branch.name }
  }
  const hold = holdOf(branch.name)
  switch (hold?.kind) {
    case undefined:
    case "landed":
      return null
    case "own":
      return { ok: false, reason: `A Safe Push of ${branch.name} is running: wait until it merges, or restart it on the current branch.`, show: branch.name }
    case "span":
      return { ok: false, reason: `A Safe Push of ${formatLandingSubject(hold)} lands all of ${branch.name}.`, show: hold.branch }
  }
}

/**
 * One line of a push of several branches, in the words of a push: "login · 2 commits · creates the remote branch",
 * "auth · 1 commit · force push" for a rebased one, with what a force push replaces or drops (`formatForcePushPart`);
 * "login · 2 of 5 commits" for a part of the branch (`sent` of its commits for the remote)
 */
export function formatStackPushLine(branch: Branch, sent: number | null = null): string {
  const action = branch.remoteStatus?.action.kind
  const parts = [branch.name]
  const ahead = branch.remoteStatus?.commitsAhead ?? 0
  if (sent !== null && sent < ahead) {
    parts.push(`${sent} of ${formatCommitCount(ahead)}`)
  } else if (ahead > 0) {
    parts.push(formatCommitCount(ahead))
  }
  if (action === "create") {
    parts.push("creates the remote branch")
  } else if (action === "force" && branch.remoteStatus != null) {
    parts.push(formatForcePushPart(branch.remoteStatus, branch.stack?.effectiveBase ?? null))
  }
  return parts.join(" · ")
}

/** "Pushes 3 branches, parents first:" and a line per push (`formatStackPushLine`); "Pushes login · 2 of 5 commits" alone */
export function describePushUpTo(steps: readonly PushUpToStep[], branchOf: (name: string) => Branch | undefined): string {
  const lines = steps.flatMap(step => {
    const branch = branchOf(step.name)
    return branch === undefined ? [] : [formatStackPushLine(branch, step.sent)]
  })
  return steps.length === 1 ? `Pushes ${lines[0] ?? steps[0]!.name}` : `Pushes ${steps.length} branches, parents first:\n${lines.join("\n")}`
}
