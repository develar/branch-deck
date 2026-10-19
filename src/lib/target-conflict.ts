import type { TargetCheck, TargetConflict } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { joinNames } from "@/lib/text"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusRequest } from "@/stores/patronus"

/**
 * What the target conflict check means for a start: `refuse` on a conflict (Patronus would fail the robot at once),
 * else `start`, with a sentence for the start's toast when the check did not run or used the last fetched target
 */
export type TargetDecision = { readonly kind: "refuse"; readonly reason: string } | { readonly kind: "start"; readonly note: string | null }

/**
 * Asks whether Patronus can rebase what a start lands onto the newest target (`check_target_conflict`: the fetch of the
 * target, then the replay of the branch onto its tip, as `bd safe-push` checks it): the branch `branchName`, or the
 * stack branch of the whole stack of the root `wholeStack`. A failed command holds nothing back: the note says so.
 */
export async function checkTargetBeforeStart(branchName: string, wholeStack: string | null): Promise<TargetDecision> {
  const baseline = branchSyncStore.getState().baselineBranch ?? ""
  const subject = wholeStack === null ? branchName : `the whole ${wholeStack} stack`
  try {
    const result = await commands.checkTargetConflict({
      ...patronusRequest(),
      baselineBranch: baseline,
      branchName: wholeStack ?? branchName,
      ...(wholeStack !== null && { stack: true }),
    })
    if (result.status === "error") {
      return { kind: "start", note: notRunNote(subject, baseline, result.error.message) }
    }
    return describeTargetCheck(result.data, subject, originalOf)
  } catch (error) {
    return { kind: "start", note: notRunNote(subject, baseline, getErrorDetails(error)) }
  }
}

/**
 * The decision of a check of `subject` ("auth", "the whole auth stack"), in the words of `bd safe-push`; `original`
 * gives the commit of the checked out branch that a copy of the virtual branch copies, as the rows show it
 */
export function describeTargetCheck(check: TargetCheck, subject: string, original: (copy: string) => string = copy => copy): TargetDecision {
  const { baseline, fetchError, outcome } = check
  const fetchNote = fetchError === null ? null : `The fetch of ${baseline} failed, so the check used the last fetched ${baseline}: ${fetchError}`
  switch (outcome.kind) {
    case "clean":
      return { kind: "start", note: fetchNote }
    case "conflict": {
      const reason = describeTargetConflict(outcome, subject, baseline, original(outcome.commit.id))
      return { kind: "refuse", reason: fetchNote === null ? reason : `${reason} ${fetchNote}.` }
    }
    case "notRun": {
      const note = notRunNote(subject, baseline, outcome.error)
      return { kind: "start", note: fetchNote === null ? note : `${note} ${fetchNote}.` }
    }
  }
}

/**
 * "Patronus can't rebase auth onto origin/master: 1a2b3c4d (Add auth) conflicts in a.txt, which origin/master changed
 * after the base of auth: 9e8d7c6b (Alice: Change a). Update Project resolves it locally; then start again."
 */
function describeTargetConflict(conflict: TargetConflict, subject: string, baseline: string, commitId: string): string {
  const more = conflict.fileCount - conflict.files.length
  const files = more > 0 ? `${conflict.files.join(", ")} and ${more} more files` : joinNames(conflict.files)
  const targetCommits = conflict.targetCommits.map(commit => `${formatShortHash(commit.id)} (${commit.author}: ${commit.subject})`).join(", ")
  const changedBy = targetCommits === "" ? "" : `: ${targetCommits}`
  return `Patronus can't rebase ${subject} onto ${baseline}: ${formatShortHash(commitId)} (${conflict.commit.subject}) conflicts in ${files}, which ${baseline} changed after the base of ${subject}${changedBy}. Update Project resolves it locally; then start again.`
}

/** The commit of the checked out branch whose copy is `copy`; `copy` itself when no row has it */
function originalOf(copy: string): string {
  for (const branch of Object.values(branchSyncStore.getState().branches)) {
    const commit = branch.commits.find(commit => commit.hash === copy)
    if (commit !== undefined) {
      return commit.originalHash
    }
  }
  return copy
}

function notRunNote(subject: string, baseline: string, error: string): string {
  return `Not checked whether Patronus can rebase ${subject} onto ${baseline}: ${error}`
}
