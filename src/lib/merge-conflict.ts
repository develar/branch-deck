import type { MissingCommit, SubjectParts } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"

/**
 * The title of a merge conflict wherever the conflict is shown (the expanded row, the sub-windows, the preview):
 * "“Add login form” does not apply cleanly onto master". The grouping prefix of the subject (Rust split it off,
 * `subjectParts`) is left out, an autosquash marker kept.
 */
export function formatConflictTitle(subject: SubjectParts, onto: string | null | undefined): string {
  return `“${subject.autosquash}${subject.rest}” does not apply cleanly onto ${onto || "the baseline"}`
}

/** A branch that has missing commits of a conflict, with their ids */
export interface MissingCommitOwner {
  branch: string
  commitIds: string[]
}

/** Where the missing commits of a conflict are */
export interface MissingCommitOwners {
  /** The branches, the one with the most commits first */
  branches: MissingCommitOwner[]
  /** The ids of the unassigned commits */
  unassigned: string[]
  /** The branch whose stack holds all of `branches`: one of them, built on all the others; null if none is */
  stackTop: string | null
}

/**
 * Groups the missing commits of a conflict by the branch they are in. `baseOf` names the branch a branch is built on
 * (`effectiveBase`), for `stackTop`.
 */
export function getMissingCommitOwners(missingCommits: readonly MissingCommit[], baseOf: (branch: string) => string | null | undefined): MissingCommitOwners {
  const byBranch = new Map<string, string[]>()
  const unassigned: string[] = []
  for (const commit of missingCommits) {
    if (commit.branch == null) {
      unassigned.push(commit.hash)
    } else {
      const ids = byBranch.get(commit.branch)
      if (ids === undefined) {
        byBranch.set(commit.branch, [commit.hash])
      } else {
        ids.push(commit.hash)
      }
    }
  }
  const branches = [...byBranch].map(([branch, commitIds]) => ({ branch, commitIds })).sort((a, b) => b.commitIds.length - a.commitIds.length || a.branch.localeCompare(b.branch))
  const names = new Set(byBranch.keys())
  const stackTop =
    branches.find(({ branch }) => {
      const chain = new Set<string>()
      let current: string | null | undefined = branch
      while (current != null && !chain.has(current)) {
        chain.add(current)
        current = baseOf(current)
      }
      return [...names].every(name => chain.has(name))
    })?.branch ?? null
  return { branches, unassigned, stackTop }
}

/**
 * The title of the explanation of a conflict: which commits outside the branch changed the conflicting files first.
 * "42 commits of IJAI-4047 and 11 branches it is built on changed these files first"
 */
export function formatMissingCommitsTitle(owners: MissingCommitOwners): string {
  const branchCount = owners.branches.length
  const unassigned = owners.unassigned.length
  const unassignedCommits = `${unassigned} unassigned ${unassigned === 1 ? "commit" : "commits"}`
  if (branchCount === 0) {
    return unassigned === 0 ? "No commit outside the branch changed these files first" : `${unassignedCommits} changed these files first`
  }
  const count = formatCommitCount(owners.branches.reduce((sum, owner) => sum + owner.commitIds.length, 0))
  const of =
    branchCount === 1
      ? owners.branches[0]!.branch
      : owners.stackTop === null
        ? `${branchCount} branches`
        : `${owners.stackTop} and ${branchCount === 2 ? "the branch" : `${branchCount - 1} branches`} it is built on`
  return `${count} of ${of} changed these files first${unassigned === 0 ? "" : `, as did ${unassignedCommits}`}`
}

/**
 * The sentence below the title: why the branch lacks them. "IJAI-3852 is built on IJAI-3828, which doesn't have them."
 * `builtOn` is null before the first sync names the baseline, undefined when the commit is copied onto something else (the
 * amended commit): then there is no such sentence.
 */
export function formatMissingCommitsReason(owners: MissingCommitOwners, branchName: string | undefined, builtOn: string | null | undefined): string | null {
  const count = owners.branches.reduce((sum, owner) => sum + owner.commitIds.length, owners.unassigned.length)
  if (branchName === undefined || builtOn === undefined || count === 0) {
    return null
  }
  return `${branchName} is built on ${builtOn || "the baseline"}, which doesn't have ${count === 1 ? "it" : "them"}.`
}
