import { useState } from "react"
import { TagIcon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { addIssueReference } from "@/hooks/useAddIssueReference"
import { useBranch } from "@/stores/branch-sync"
import { closeInline } from "@/stores/inline-row-action"

// flexible issue formats: ABC-123, ISSUE-456, etc.
const issueReferencePattern = /^[A-Z]+-\d+$/

export function InlineIssueReferenceInput({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const [issueReference, setIssueReference] = useState("")

  const commitCount = branch?.commitCount ?? 0
  const isValid = issueReferencePattern.test(issueReference)
  const errorMessage = issueReference && !isValid ? "Issue reference must be in format like ABC-123" : undefined

  return (
    <InlineInputDialog
      value={issueReference}
      onValueChange={setIssueReference}
      placeholder={`Issue reference for ${branchName} (${commitCount} ${commitCount === 1 ? "commit" : "commits"})`}
      validationMessage={errorMessage}
      validationColor={errorMessage ? "error" : "primary"}
      validationTextClassName={errorMessage ? "text-destructive" : ""}
      canSubmit={isValid}
      title={`Add an issue reference to ${branchName}`}
      submitText="Add"
      dataTestId="inline-issue-input"
      leadingIcon={<TagIcon className="size-4 text-muted-foreground" />}
      helpText="The issue reference will be added after the branch prefix in commit messages."
      onSubmit={() => {
        if (branch !== undefined) {
          void addIssueReference(issueReference, branch)
        }
      }}
      onCancel={closeInline}
    />
  )
}
