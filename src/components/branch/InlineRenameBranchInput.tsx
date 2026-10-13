import { useState } from "react"
import { PencilIcon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getBranch, useBranch } from "@/stores/branch-sync"
import { closeInline, withPostSubmit } from "@/stores/inline-row-action"

/**
 * Renames a branch by moving all its commits to the new name; an issue branch gets an explicit prefix on every commit
 * (the issue key stays in the messages). An existing name merges the branches.
 */
export function InlineRenameBranchInput({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const isIssueBranch = branch?.origin === "Issue"
  const [name, setName] = useState(isIssueBranch ? "" : branchName)
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(name)
  const isUnchanged = sanitizedBranchName === branchName
  const existing = useBranch(sanitizedBranchName) !== undefined && !isUnchanged

  const commitCount = branch?.commitCount ?? 0
  let helpText = isIssueBranch ? `Adds the prefix to ${commitCount === 1 ? "the commit" : `all ${commitCount} commits`}; ${branchName} stays in the messages` : undefined
  if (isValid && existing) {
    helpText = `Moves the commits into the existing branch ${sanitizedBranchName}`
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
      title={isIssueBranch ? `Name the Branch ${branchName}` : `Rename ${branchName}`}
      description={`Rename form for the ${branchName} branch`}
      submitText={existing ? "Move" : (isIssueBranch ? "Name" : "Rename")}
      dataTestId="inline-rename-input"
      leadingIcon={<PencilIcon className="size-4 text-muted-foreground" />}
      helpText={helpText}
      onSubmit={() => {
        const target = sanitizedBranchName
        withPostSubmit(async () => {
          const current = getBranch(branchName)
          if (current !== undefined) {
            await moveCommits(current.commits, target)
          }
        })
      }}
      onCancel={closeInline}
    />
  )
}
