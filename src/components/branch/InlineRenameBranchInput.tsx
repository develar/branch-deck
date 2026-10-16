import { useState } from "react"
import { PencilIcon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import { renameBranch } from "@/hooks/useStackActions"
import { getStackDescendants, useBranch, useStacksEnabled } from "@/stores/branch-sync"
import { closeInline, withRowProgress } from "@/stores/inline-row-action"

/**
 * Renames a branch (`rename_branch`): a prefix branch in the prefixes of its commits; an issue branch and an issue
 * group in the issue groups, without a rewrite (a stacked one by a prefix), and so are the issue groups of a prefix
 * branch. An existing name merges the branches. With stacked branches on, the branches stacked on it follow: the name
 * changes in their prefixes too.
 */
export function InlineRenameBranchInput({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const isIssueBranch = branch?.origin === "Issue"
  const isIssueGroup = branch?.origin === "IssueGroup"
  const [name, setName] = useState(isIssueBranch ? "" : branchName)
  const stacks = useStacksEnabled()
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(name, { stacks, leafOnly: true })
  const followers = stacks ? getStackDescendants(branchName) : []
  const isUnchanged = sanitizedBranchName === branchName
  const existing = useBranch(sanitizedBranchName) !== undefined && !isUnchanged

  let helpText = isIssueBranch ? `Names the branch without a rewrite; the next commits of ${branchName} go there too` : undefined
  if (isIssueGroup) {
    helpText = "Renames the issue group: the commit messages stay as they are"
  }
  if (isValid && existing) {
    helpText = `Moves the commits into the existing branch ${sanitizedBranchName}`
  } else if (followers.length > 0) {
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
      submitText={existing ? "Move" : isIssueBranch ? "Name" : "Rename"}
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
        void withRowProgress(branchName, () => renameBranch(branchName, target), { processingMessage, keepFormOnFailure: true })
      }}
      onCancel={closeInline}
    />
  )
}
