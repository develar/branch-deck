import { useState } from "react"
import { PencilIcon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import { moveCommits } from "@/hooks/useMoveCommits"
import { renameBranch } from "@/hooks/useStackActions"
import { getBranch, getStackDescendants, useBranch, useStacksEnabled } from "@/stores/branch-sync"
import { closeInline, withRowProgress } from "@/stores/inline-row-action"

/**
 * Renames a branch by moving all its commits to the new name; an issue branch gets an explicit prefix on every commit
 * (the issue key stays in the messages). An existing name merges the branches. With stacked branches on, the branches
 * stacked on it follow: the name changes in their prefixes too.
 */
export function InlineRenameBranchInput({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const isIssueBranch = branch?.origin === "Issue"
  const [name, setName] = useState(isIssueBranch ? "" : branchName)
  const stacks = useStacksEnabled()
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(name, { stacks, leafOnly: true })
  const followers = stacks ? getStackDescendants(branchName) : []
  const isUnchanged = sanitizedBranchName === branchName
  const existing = useBranch(sanitizedBranchName) !== undefined && !isUnchanged

  const commitCount = branch?.commitCount ?? 0
  let helpText = isIssueBranch ? `Adds the prefix to ${commitCount === 1 ? "the commit" : `all ${commitCount} commits`}; ${branchName} stays in the messages` : undefined
  if (isValid && existing) {
    helpText = `Moves the commits into the existing branch ${sanitizedBranchName}`
  }
  else if (followers.length > 0) {
    helpText = `${followers.join(", ")} stay${followers.length === 1 ? "s" : ""} stacked on it`
  }
  return (
    <InlineInputDialog
      value={name}
      onValueChange={setName}
      placeholder={isIssueBranch ? `Branch name for ${branchName}` : "New branch name"}
      validationMessage={validationState.message}
      validationColor={validationState.color ?? "primary"}
      validationTextClassName={validationState.textClass}
      canSubmit={isValid && !isUnchanged}
      title={isIssueBranch ? `Name the branch ${branchName}` : `Rename ${branchName}`}
      submitText={existing ? "Move" : (isIssueBranch ? "Name" : "Rename")}
      dataTestId="inline-rename-input"
      leadingIcon={<PencilIcon className="size-4 text-muted-foreground" />}
      helpText={helpText}
      onSubmit={() => {
        const target = sanitizedBranchName
        let processingMessage = isIssueBranch ? `Naming the branch ${target}…` : `Renaming ${branchName} to ${target}…`
        if (existing) {
          processingMessage = `Moving the commits of ${branchName} to ${target}…`
        }
        // a failure keeps the form with the typed name
        void withRowProgress(branchName, async () => {
          if (stacks) {
            return renameBranch(branchName, target)
          }
          const current = getBranch(branchName)
          return current === undefined || await moveCommits(current.commits, target)
        }, { processingMessage, keepFormOnFailure: true })
      }}
      onCancel={closeInline}
    />
  )
}
