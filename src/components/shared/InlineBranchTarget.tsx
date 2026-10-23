import { useEffect, useRef, useState } from "react"
import { GitBranchIcon } from "lucide-react"
import { AIStatusIndicator } from "@/components/ai/AIStatusIndicator"
import { BranchNameSuggestions } from "@/components/ai/BranchNameSuggestions"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import type { InlineInputDialogHandle } from "@/components/shared/InlineInputDialog"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"
import { useBranchNameValidation } from "@/hooks/useBranchNameValidation"
import { moveCommits } from "@/hooks/useMoveCommits"
import { useMovePreview } from "@/hooks/useMovePreview"
import type { Commit, MoveCommitsResult } from "@/lib/bindings"
import { formatCommitCount, groupFor } from "@/lib/commit"
import { countMoved, describeMove, formatKeys } from "@/lib/move-summary"
import { useBranchSync, useMoveTargets, useStackParents, useStacksEnabled } from "@/stores/branch-sync"
import type { Branch } from "@/stores/branch-sync"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/operations"
import { useSelectedProjectPath } from "@/stores/repository"

/** Existing branches offered as chips */
const MAX_TARGET_CHIPS = 6

export interface InlineBranchTargetProps {
  commits: readonly Commit[]
  /** The branches the commits are in: one for commits of a branch, several for selected branches, none for the unassigned commits */
  sourceBranches: readonly string[]
  isActive: boolean
  onCancel: () => void
  onSuccess: () => void
}

/**
 * Inline "Move to…" form: one name input; an existing branch name moves the commits there, a new name creates the
 * branch. Existing branches (the same stack and the same issue first) and AI suggestions are offered below the input.
 * With stacked branches on, `auth/login` names the branch `login` on `auth`, and a branch is looked up by its leaf.
 * A dry run of the move says what it does: the issues that go there by their key, without a rewrite, and the commits
 * that get the prefix; a source branch already pushed keeps its remote branch.
 */
/** The surface of the move this form starts */
const MOVE_TARGET_SURFACE = "move-target"

export function InlineBranchTarget({ commits, sourceBranches, isActive, onCancel, onSuccess }: InlineBranchTargetProps) {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const sourceBranch = sourceBranches.length === 1 ? sourceBranches[0]! : null
  const targets = useMoveTargets(sourceBranch, MAX_TARGET_CHIPS)
  // the move runs from here: the form shows its progress until its sync shows the commits moved
  useIndicatorClaim(MOVE_TARGET_SURFACE)
  const isMoving = useSurfaceBusy(MOVE_TARGET_SURFACE) !== null

  const [branchName, setBranchName] = useState("")
  const stacks = useStacksEnabled()
  const stackParents = useStackParents()
  const { sanitizedBranchName, validationState, isValid } = useBranchNameValidation(branchName, { stacks })
  // with stacked branches the name may carry a parent: `auth/login` is the branch `login`; a bare name takes its
  // stack parent
  const target = stacks ? groupFor(`(${sanitizedBranchName})`, true, {}, stackParents) : null
  const leaf = target?.branch ?? sanitizedBranchName
  const parent = target?.parent ?? null
  const branches = useBranchSync(state => state.branches)
  const existingBranch = branches[leaf]
  const existing = existingBranch !== undefined
  const isSource = leaf === sourceBranch
  const preview = useMovePreview(commits, sanitizedBranchName, isValid && !isSource && sanitizedBranchName !== "")
  const isAlreadyThere = preview.result !== null && countMoved(preview.result) === 0

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
    const success = await moveCommits(commits, sanitizedBranchName, { surface: MOVE_TARGET_SURFACE })
    if (success) {
      setBranchName("")
      onSuccess()
    }
  }

  const cancel = () => {
    setBranchName("")
    onCancel()
  }

  const commitCount = formatCommitCount(commits.length)
  let helpText: string | undefined
  if (isValid && !isSource) {
    if (!stacks) {
      helpText = existing ? `Moves ${commitCount} to the existing branch ${sanitizedBranchName}` : `Creates the branch ${sanitizedBranchName} with ${commitCount}`
    } else {
      const existingParent = existingBranch?.stack?.declaredParent ?? null
      const onParent = parent ?? existingParent
      const on = onParent === null ? "" : ` (on ${onParent})`
      if (!existing) {
        helpText = parent === null ? `Creates ${leaf} with ${commitCount}` : `Creates ${leaf} on ${parent} with ${commitCount}`
      } else if (parent !== null && existingParent !== parent) {
        // the path writes the stack parent: the whole branch moves onto the new parent
        helpText = `Moves ${commitCount} to ${leaf} and stacks it on ${parent}${existingParent === null ? "" : ` instead of ${existingParent}`}`
      } else {
        helpText = `Moves ${commitCount} to ${leaf}${on}`
      }
    }
    // the move rewrites commits only where it must: the help says what it does, not only the undo toast
    const effect = preview.result === null ? undefined : describeEffect(preview.result, leaf, commits.length)
    if (effect !== undefined && helpText !== undefined) {
      helpText = `${helpText}: ${effect}`
    }
  }
  const baseline = useBaselineOnRemote()
  const { warnings, notes } = isValid && !isSource ? getLeftBehind(commits, sourceBranches, leaf, branches, preview.result, baseline) : NOTHING_LEFT
  let validationMessage = validationState.message
  if (isSource || isAlreadyThere) {
    validationMessage = "The commits are already in this branch"
  } else if (preview.error !== null) {
    validationMessage = preview.error
  }
  const isRefused = isSource || isAlreadyThere || preview.error !== null
  return (
    <div className="relative overflow-hidden" data-testid="inline-branch-target">
      {isMoving && <InlineRowProgressBar />}
      <LockWaitNotice surface={MOVE_TARGET_SURFACE} />
      <InlineInputDialog
        ref={dialogRef}
        title={`Move ${commitCount} to a branch`}
        value={branchName}
        onValueChange={setBranchName}
        placeholder={`Branch for ${commitCount}: an existing or a new name`}
        canSubmit={isValid && !isRefused && !isMoving}
        validationMessage={validationMessage}
        validationColor={isRefused ? "error" : (validationState.color ?? "primary")}
        validationTextClassName={isRefused ? "text-destructive" : validationState.textClass}
        helpText={
          warnings.length === 0 && notes.length === 0 ? (
            helpText
          ) : (
            <div className="flex flex-col gap-1">
              {helpText !== undefined && <span>{helpText}</span>}
              {notes.map(note => (
                <span key={note}>{note}</span>
              ))}
              {warnings.map(warning => (
                <span key={warning} className="text-warning" data-testid="branch-target-warning">
                  {warning}
                </span>
              ))}
            </div>
          )
        }
        submitText={existing ? "Move" : "Create"}
        loading={isMoving}
        leadingIcon={<GitBranchIcon className="size-4 text-muted-foreground" />}
        extraActions={
          <div className="flex items-center gap-3">
            <AIStatusIndicator />
            <div className="h-4 w-px bg-border" />
          </div>
        }
        afterControls={
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
            <BranchNameSuggestions repositoryPath={repositoryPath} commits={commits} isActive={isActive} selectedName={sanitizedBranchName} onSelect={choose} />
          </>
        }
        onSubmit={() => void submit()}
        onCancel={cancel}
      />
    </div>
  )
}

/**
 * What the move does to the commits, from its dry run: "rewrites them to start with (auth)" when every commit gets the
 * prefix, else the issues that go by their key and the prefixes that change
 */
function describeEffect(result: MoveCommitsResult, target: string, count: number): string | undefined {
  const isAllPrefixed = result.commits.length === count && result.commits.every(commit => commit.change === "Prefixed")
  if (isAllPrefixed) {
    return `rewrites ${count === 1 ? "it" : "them"} to start with (${target})`
  }
  return describeMove(result, target, false)
}

interface LeftBehind {
  /** The remote branch and pull request of a source branch that the move empties */
  warnings: string[]
  /** The branches stacked on a parent that the move empties: they stay stacked on it */
  notes: string[]
}

const NOTHING_LEFT: LeftBehind = { warnings: [], notes: [] }

/** What the move leaves behind: the remote branches of the source branches it empties, their stacked children */
function getLeftBehind(
  commits: readonly Commit[],
  sourceBranches: readonly string[],
  target: string,
  branches: Readonly<Record<string, Branch>>,
  result: MoveCommitsResult | null,
  baseline: string,
): LeftBehind {
  const moved = new Set(commits.map(commit => commit.originalHash))
  const warnings: string[] = []
  const pushed = sourceBranches.filter(name => {
    const branch = branches[name]
    return name !== target && branch?.remoteStatus?.exists === true && branch.commits.every(commit => moved.has(commit.originalHash))
  })
  if (pushed.length > 0) {
    warnings.push(
      pushed.length === 1 ? `${pushed[0]!} is pushed: its remote branch and pull request stay` : `${formatKeys(pushed)} are pushed: their remote branches and pull requests stay`,
    )
  }

  // the words of the Ungroup and Unapply confirmations (`StackChildrenNote`)
  const byParent = new Map<string, string[]>()
  for (const child of result?.orphanedChildren ?? []) {
    const parent = branches[child]?.stack?.declaredParent
    if (parent != null) {
      byParent.set(parent, [...(byParent.get(parent) ?? []), child])
    }
  }
  const notes = [...byParent].map(([parent, children]) =>
    children.length === 1
      ? `${children[0]!} stays stacked on ${parent} and is built on ${baseline} until ${parent} has commits again`
      : `${formatKeys(children)} stay stacked on ${parent} and are built on ${baseline} until ${parent} has commits again`,
  )
  return { warnings, notes }
}
