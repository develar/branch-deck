import { formatCommitCount } from "@/lib/commit"
import { dropsRemoteCommits, getPushAction } from "@/lib/push-summary"
import { formatForcePushPart } from "@/lib/remote-status"
import { needsPush, stackAncestors } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"

/** One push of a Push Up to Here, root first */
export interface PushUpToStep {
  readonly name: string
  /** The commit to push instead of the tip (`PushBranchParams.tip`); undefined pushes the whole branch */
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
   * The branch whose running Safe Push holds `name` (`PushHold`): `name` itself, or a branch built on it that lands it
   * whole; null while none does
   */
  readonly landingOf: (name: string) => string | null
}

/**
 * Push Up to Here of the commit `originalHash` of the branch `name`: the branch up to the commit, after every branch it
 * is built on that has work for the remote, whole (the branch is built on their tips), root first. Only a commit the
 * remote lacks on a branch whose remote is an older state of it (not on the remote, or a fast-forward) can be a cut: a
 * part of a branch rewritten since its push would rewind its pull request. An ancestor that can't be pushed (blocked,
 * failed to sync) or asks first (its push drops remote commits), and a branch a running Safe Push lands, stop it.
 * Null for a commit the remote has: it offers nothing.
 */
export function pushUpToPlan(name: string, originalHash: string, { branchOf, landingOf }: PushUpToSources): PushUpToPlan | null {
  const branch = branchOf(name)
  const commit = branch?.commits.find(it => it.originalHash === originalHash)
  const remote = branch?.remoteStatus
  if (branch === undefined || commit?.hash === undefined || remote == null) {
    return null
  }
  const at = remote.unpushedCommits.indexOf(commit.hash)
  if (at < 0) {
    return null
  }
  const own = blocker(branch, landingOf)
  if (own !== null) {
    return own
  }
  if (remote.exists && !remote.fastForward) {
    return { ok: false, reason: `${name} was rewritten since its push: it pushes whole.` }
  }
  const steps: PushUpToStep[] = []
  for (const ancestor of stackAncestors(name, branchOf)) {
    const stop = blocker(ancestor, landingOf)
    if (stop !== null) {
      return stop
    }
    if (ancestor.remoteStatus != null && needsPush(ancestor) && dropsRemoteCommits(ancestor.remoteStatus)) {
      const behind = ancestor.remoteStatus.commitsBehind
      return {
        ok: false,
        reason: `${ancestor.name}'s push drops ${behind === 1 ? "1 remote commit" : `${behind} remote commits`}: Force Push ${ancestor.name} first.`,
        show: ancestor.name,
      }
    }
    if (needsPush(ancestor)) {
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
 * Why the branch can't be pushed in a Push Up to Here: blocked, failed to sync, its own Safe Push running, landed whole
 * by the running Safe Push of a branch built on it
 */
function blocker(branch: Branch, landingOf: (name: string) => string | null): PushUpToRefusal | null {
  if (branch.status === "Blocked" || branch.hasError) {
    return { ok: false, reason: `${branch.name} can't be pushed: ${branch.statusText === "" ? "it failed to sync" : branch.statusText}.`, show: branch.name }
  }
  const landing = landingOf(branch.name)
  if (landing === branch.name) {
    return { ok: false, reason: `A Safe Push of ${branch.name} is running: wait until it merges, or push and restart it.`, show: landing }
  }
  if (landing !== null) {
    return { ok: false, reason: `A Safe Push of ${landing} lands all of ${branch.name}.`, show: landing }
  }
  return null
}

/**
 * One line of a push of several branches, in the words of a push: "login · 2 commits · creates the remote branch",
 * "auth · 1 commit · force push" for a rebased one, with what a force push replaces or drops (`formatForcePushPart`);
 * "login · 2 of 5 commits" for a part of the branch (`sent` of its commits for the remote)
 */
export function formatStackPushLine(branch: Branch, sent: number | null = null): string {
  const action = getPushAction(branch)
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
