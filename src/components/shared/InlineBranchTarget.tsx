import { useEffect, useRef, useState } from "react"
import { GitBranchIcon } from "lucide-react"
import { AIStatusIndicator } from "@/components/ai/AIStatusIndicator"
import { BranchNameSuggestions } from "@/components/ai/BranchNameSuggestions"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import type { InlineInputDialogHandle } from "@/components/shared/InlineInputDialog"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import { moveCommits } from "@/hooks/useMoveCommits"
import type { Commit } from "@/lib/bindings"
import { groupFor } from "@/lib/commit"
import { useBranch, useMoveTargets, useStacksEnabled } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"

/** Existing branches offered as chips */
const MAX_TARGET_CHIPS = 6

export interface InlineBranchTargetProps {
  commits: readonly Commit[]
  /** The branch the commits are in; null for the unassigned commits */
  sourceBranch: string | null
  isActive: boolean
  onCancel: () => void
  onSuccess: () => void
}

/**
 * Inline "Move to…" form: one name input; an existing branch name moves the commits there, a new name creates the
 * branch. Existing branches (the same stack and the same issue first) and AI suggestions are offered below the input.
 * With stacked branches on, `auth/login` names the branch `login` on `auth`, and a branch is looked up by its leaf.
 */
export function InlineBranchTarget({ commits, sourceBranch, isActive, onCancel, onSuccess }: InlineBranchTargetProps) {
  const repositoryPath = useSelectedProject()?.path ?? ""
  const effectiveBranchPrefix = useEffectiveBranchPrefix()
  const targets = useMoveTargets(sourceBranch, MAX_TARGET_CHIPS)
  const [isMoving, setIsMoving] = useState(false)

  const [branchName, setBranchName] = useState("")
  const stacks = useStacksEnabled()
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(branchName, { stacks })
  // with stacked branches the name may carry a parent: `auth/login` is the branch `login`
  const target = stacks ? groupFor(`(${sanitizedBranchName})`, true) : null
  const leaf = target?.branch ?? sanitizedBranchName
  const parent = target?.parent ?? null
  const existingBranch = useBranch(leaf)
  const existing = existingBranch !== undefined
  const isSource = leaf === sourceBranch

  const dialogRef = useRef<InlineInputDialogHandle>(null)
  const hasAutoPopulatedRef = useRef(false)
  // select the text once the chosen name is rendered, for easy override
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

  // chip clicks and auto-population of the first AI suggestion
  const choose = (name: string, isAuto = false) => {
    if (isAuto) {
      if (!isActive || hasAutoPopulatedRef.current || branchName !== "") {
        return
      }
      hasAutoPopulatedRef.current = true
    }
    selectAfterRenderRef.current = true
    setBranchName(name)
  }

  const submit = async () => {
    setIsMoving(true)
    const success = await moveCommits(commits, sanitizedBranchName)
    setIsMoving(false)
    if (success) {
      setBranchName("")
      onSuccess()
    }
  }

  const cancel = () => {
    setBranchName("")
    onCancel()
  }

  const commitCount = commits.length === 1 ? "1 commit" : `${commits.length} commits`
  let helpText: string | undefined
  if (isValid && !isSource) {
    if (!stacks) {
      helpText = existing ? `Moves ${commitCount} to the existing branch ${sanitizedBranchName}` : `Creates the branch ${sanitizedBranchName} with ${commitCount}`
    }
    else {
      const existingParent = existingBranch?.stack?.declaredParent ?? null
      const onParent = parent ?? existingParent
      const on = onParent === null ? "" : ` (on ${onParent})`
      if (!existing) {
        helpText = parent === null ? `Creates ${leaf} with ${commitCount}` : `Creates ${leaf} on ${parent} with ${commitCount}`
      }
      else if (parent !== null && existingParent !== parent) {
        helpText = `${leaf} is ${existingParent === null ? "not stacked" : `on ${existingParent}`}: its parents would conflict`
      }
      else {
        helpText = `Moves ${commitCount} to ${leaf}${on}`
      }
    }
  }
  return (
    <div className="relative overflow-hidden" data-testid="inline-branch-target">
      {isMoving && <InlineRowProgressBar />}
      <InlineInputDialog
        ref={dialogRef}
        title={`Move ${commitCount} to a branch`}
        value={branchName}
        onValueChange={setBranchName}
        placeholder={`Branch for ${commitCount}: an existing or a new name`}
        canSubmit={isValid && !isSource && !isMoving}
        validationMessage={isSource ? "The commits are already in this branch" : validationState.message}
        validationColor={isSource ? "error" : (validationState.color ?? "primary")}
        validationTextClassName={isSource ? "text-destructive" : validationState.textClass}
        helpText={helpText}
        submitText={existing ? "Move" : "Create"}
        loading={isMoving}
        leadingIcon={<GitBranchIcon className="size-4 text-muted-foreground" />}
        extraActions={(
          <div className="flex items-center gap-3">
            <AIStatusIndicator />
            <div className="h-4 w-px bg-border" />
          </div>
        )}
        afterControls={(
          <>
            {targets.length > 0 && (
              <div className="flex flex-col gap-2" data-testid="branch-target-chips">
                <p className="text-xs text-muted-foreground">Existing branches:</p>
                <div className="flex flex-wrap gap-2">
                  {targets.map(name => (
                    <ChoiceChip key={name} chosen={name === sanitizedBranchName} onClick={() => choose(name)}>
                      <GitBranchIcon data-icon="inline-start" />
                      {name}
                    </ChoiceChip>
                  ))}
                </div>
              </div>
            )}
            <BranchNameSuggestions
              repositoryPath={repositoryPath}
              branchPrefix={effectiveBranchPrefix}
              commits={commits}
              isActive={isActive}
              selectedName={sanitizedBranchName}
              onSelect={choose}
            />
          </>
        )}
        onSubmit={() => void submit()}
        onCancel={cancel}
      />
    </div>
  )
}
