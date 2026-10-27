// The page's words of a rewrite of the engine (`Intent` of the kind `rewrite`, `operations::rewrite`): its row while it
// runs ("Unstacking beta"), the sentence of an operation that holds what another needs ("the unstack of beta"), and the
// title of its failure in Activity ("Cannot unstack beta"). Rust decides the rewrite; these only word it.
import type { RewriteIntent } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"

/** What a rewrite says: its row while it runs, the phrase of a sentence, the title of its failure */
export interface RewriteWords {
  readonly label: string
  readonly phrase: string
  readonly failure: string
}

/** The words of `rewrite` */
export function presentRewrite(rewrite: RewriteIntent): RewriteWords {
  switch (rewrite.rewrite) {
    case "move": {
      const count = formatCommitCount(rewrite.commitIds.length)
      return rewrite.targetBranch === null
        ? words(`Moving ${count} back`, `a move of ${count} back`, "Cannot move the commits back")
        : words(`Moving ${count} to ${rewrite.targetBranch}`, `a move of ${count} to ${rewrite.targetBranch}`, `Cannot move the commits to ${rewrite.targetBranch}`)
    }
    case "ungroup":
      return words(`Ungrouping ${rewrite.branchName}`, `the ungroup of ${rewrite.branchName}`, `Cannot ungroup ${rewrite.branchName}`)
    case "rename":
      return words(`Renaming ${rewrite.branchName} to ${rewrite.newName}`, `the rename of ${rewrite.branchName}`, `Cannot rename ${rewrite.branchName}`)
    case "restack":
      return words(`Stacking ${rewrite.branchName} on ${rewrite.parent}`, `the restack of ${rewrite.branchName}`, `Cannot stack ${rewrite.branchName} on ${rewrite.parent}`)
    case "unstack":
      return words(`Unstacking ${rewrite.branchName}`, `the unstack of ${rewrite.branchName}`, `Cannot unstack ${rewrite.branchName}`)
    case "park": {
      const names = rewrite.branches.map(line => line.branchName).join(", ")
      return words(`Parking ${names}`, `the park of ${names}`, `Cannot park ${names}`)
    }
    case "unpark": {
      const names = rewrite.branchNames.join(", ")
      return words(`Unparking ${names}`, `the unpark of ${names}`, `Cannot unpark ${names}`)
    }
    case "unapply":
      return words(`Unapplying ${rewrite.branchName}`, `the unapply of ${rewrite.branchName}`, `Cannot unapply ${rewrite.branchName}`)
    case "reapply":
      return words(`Reapplying ${rewrite.branchName}`, `the reapply of ${rewrite.branchName}`, `Cannot reapply ${rewrite.branchName}`)
    case "squash": {
      const count = formatCommitCount(rewrite.commitIds.length)
      return words(`Squashing ${count}`, `a squash of ${count}`, `Cannot squash ${count}`)
    }
    case "squashFixups":
      return words("Squashing the fixups", "a squash of the fixups", "Cannot squash the fixups")
    case "reword":
      return words("Editing the message", "an edit of a message", "Cannot edit the message")
    case "amend":
      return words(`Amending ${rewrite.branchName}`, `an amend of ${rewrite.branchName}`, `Cannot amend ${rewrite.branchName}`)
    case "addIssueReference":
      return words(
        `Adding ${rewrite.issueReference} to ${rewrite.branchName}`,
        `the issue reference of ${rewrite.branchName}`,
        `Cannot add ${rewrite.issueReference} to ${rewrite.branchName}`,
      )
    case "applyConflictFix":
      return words("Applying the fix", "the fix of a conflict", "Cannot apply the fix")
    case "deleteArchived":
    case "deleteUnapplied":
      return words(`Deleting ${rewrite.branchName}`, `the delete of ${rewrite.branchName}`, `Cannot delete ${rewrite.branchName}`)
    case "pickRemote":
      return words(
        `Picking the remote commits of ${rewrite.branchName}`,
        `the pick of the remote commits of ${rewrite.branchName}`,
        `Cannot pick the remote commits of ${rewrite.branchName}`,
      )
    case "undo":
      return words("Undoing", "an undo", "Cannot undo")
  }
}

function words(label: string, phrase: string, failure: string): RewriteWords {
  return { label, phrase, failure }
}
