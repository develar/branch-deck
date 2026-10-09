import { useEffect, useRef, useState } from "react"
import { GitBranchIcon } from "lucide-react"
import { AIStatusIndicator } from "@/components/ai/AIStatusIndicator"
import { BranchNameSuggestions } from "@/components/ai/BranchNameSuggestions"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import type { InlineInputDialogHandle } from "@/components/shared/InlineInputDialog"
import { useBranchCreation } from "@/hooks/useBranchCreation"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import type { Commit } from "@/lib/bindings"
import { syncBranches } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject, useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"

export interface InlineBranchCreatorProps {
  selectedCommits: readonly Commit[]
  isActive: boolean
  dialogTitle?: string
  dialogDescription?: string
  onCancel: () => void
  onSuccess: () => void
}

/** Inline form creating a branch from the selected unassigned commits; rendered in place above the commit table. */
export function InlineBranchCreator({
  selectedCommits,
  isActive,
  dialogTitle = "Create Branch",
  dialogDescription = "Create a new branch from selected commits",
  onCancel,
  onSuccess,
}: InlineBranchCreatorProps) {
  const repositoryPath = useSelectedProject()?.path ?? ""
  const effectiveBranchPrefix = useEffectiveBranchPrefix()
  const { createBranch } = useBranchCreation()
  // the fixed processing key used by `createBranch`
  const isCreating = useIsRowProcessing("branch-creation")

  const [branchName, setBranchName] = useState("")
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(branchName)

  const dialogRef = useRef<InlineInputDialogHandle>(null)
  const hasAutoPopulatedRef = useRef(false)
  // select the text once the suggested name is rendered, for easy override
  const selectAfterRenderRef = useRef(false)

  useEffect(() => {
    if (isActive) {
      hasAutoPopulatedRef.current = false
    }
  }, [isActive])

  useEffect(() => {
    if (selectAfterRenderRef.current) {
      selectAfterRenderRef.current = false
      dialogRef.current?.selectText()
      dialogRef.current?.focusInput()
    }
  }, [branchName])

  // manual clicks and auto-population of the first suggestion
  const handleSuggestionSelect = (name: string, isAuto = false) => {
    if (isAuto) {
      if (!isActive || hasAutoPopulatedRef.current || branchName !== "") {
        return
      }
      hasAutoPopulatedRef.current = true
    }
    selectAfterRenderRef.current = true
    setBranchName(name)
  }

  const handleCreateBranch = async () => {
    // captured before the input is cleared
    const effectiveBranchName = sanitizedBranchName
    const success = await createBranch({
      repositoryPath: getSelectedProject()?.path ?? "",
      branchName: effectiveBranchName,
      commitIds: selectedCommits.map(commit => commit.originalHash),
    })
    if (!success) {
      return
    }

    setBranchName("")
    onSuccess()

    await syncBranches({
      targetBranchName: effectiveBranchName,
      autoExpand: true,
      autoScroll: true,
    })
  }

  const cancel = () => {
    setBranchName("")
    onCancel()
  }

  return (
    <InlineInputDialog
      ref={dialogRef}
      title={dialogTitle}
      description={dialogDescription}
      value={branchName}
      onValueChange={setBranchName}
      placeholder={`Branch name for ${selectedCommits.length} ${selectedCommits.length === 1 ? "commit" : "commits"}`}
      canSubmit={isValid && !isCreating}
      validationMessage={validationState.message}
      validationColor={validationState.color ?? "primary"}
      validationTextClassName={validationState.textClass}
      submitText="Create"
      leadingIcon={<GitBranchIcon className="size-4 text-muted-foreground" />}
      extraActions={(
        <div className="flex items-center gap-3">
          <AIStatusIndicator />
          <div className="h-4 w-px bg-border" />
        </div>
      )}
      afterControls={(
        <BranchNameSuggestions
          repositoryPath={repositoryPath}
          branchPrefix={effectiveBranchPrefix}
          commits={selectedCommits}
          isActive={isActive}
          onSelect={handleSuggestionSelect}
        />
      )}
      onSubmit={() => void handleCreateBranch()}
      onCancel={cancel}
    />
  )
}
