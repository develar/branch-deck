import type { ReactNode } from "react"
import { CopyCheckIcon, CopyIcon, RotateCwIcon, WandSparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { HintList } from "@/components/conflict/ConflictResolutionHint"
import { Callout } from "@/components/shared/Callout"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { useAgentCliCommand } from "@/hooks/useAgentCliCommand"
import { useCopyToClipboard } from "@/hooks/useCopyToClipboard"
import type { FixComponent, MergeConflictInfo } from "@/lib/bindings"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { branchesOfFix, componentFixing, describeFixGrouping, describeFixResult, describeFixStep } from "@/lib/conflict-fix"
import { buildConflictPrompt } from "@/lib/conflict-fix-prompt"
import { formatMissingCommitsTitle } from "@/lib/merge-conflict"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import { formatPendingFact, formatViewFailure } from "@/presenters/next-step"
import { APPLY_FIX_SURFACE, applyConflictFix, requestConflictFix, useConflictFix, useSubjectOf } from "@/stores/conflict-fix"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/operations"
import { usePatronusPending, usePatronusViewFailure, usePushHolds } from "@/stores/patronus"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"
import { useSelectedProject } from "@/stores/repository"

export interface ConflictFixPanelProps {
  conflict: MergeConflictInfo
  branchName: string
  builtOn: string | null
  owners: MissingCommitOwners
}

/**
 * The fix of a sync conflict (spec/branch/conflict-fix.spec.md), a callout below the explanation: looked for when the
 * conflict is shown, by trial builds; the steps in the words of the Move action, what they fix and empty, and Apply Fix
 * with Undo. Without a fix, the prompt that hands the conflict to a coding agent.
 */
export function ConflictFixPanel(props: ConflictFixPanelProps) {
  const { status, fix, error } = useConflictFix(true)
  const component = fix === null ? undefined : componentFixing(fix.components, props.branchName)

  if (status === "error") {
    return (
      <FixCallout title={`Cannot look for a fix: ${error ?? "unknown error"}`} state="error">
        <div className="flex flex-wrap items-center gap-2">
          <LookAgain />
          <CopyConflictPrompt {...props} />
        </div>
      </FixCallout>
    )
  }
  if (status !== "done" || fix === null) {
    return <FixCallout title="Looking for a fix…" state="searching" />
  }
  if (component === undefined) {
    const complete = fix.search === "complete"
    return (
      <FixCallout title={complete ? "Branch Deck found no regroup that applies this commit cleanly" : "Branch Deck stopped looking for a fix"} state="none">
        <p>Move its change with the menu of the commit (Move to…), or hand the conflict to a coding agent.</p>
        <div className="flex flex-wrap items-center gap-2">
          {!complete && <LookAgain />}
          <CopyConflictPrompt {...props} />
        </div>
      </FixCallout>
    )
  }
  return <FixSteps component={component} />
}

function FixCallout({ title, state, children }: { title: string; state: "searching" | "found" | "none" | "error"; children?: ReactNode }) {
  return (
    <Callout
      tone="info"
      icon={WandSparklesIcon}
      title={
        state === "searching" ? (
          <span className="inline-flex items-center gap-2">
            {title}
            <Spinner className="size-3.5" />
          </span>
        ) : (
          title
        )
      }
      data-testid="conflict-fix"
      data-state={state}
      aria-busy={state === "searching" || undefined}
    >
      {children}
    </Callout>
  )
}

function FixSteps({ component }: { component: FixComponent }) {
  const holds = usePushHolds()
  // what the robots hold is known only once Patronus answered with its view: the fix waits for it, never on "none"
  const holdsPending = usePatronusPending()
  const viewFailure = usePatronusViewFailure()
  const subjectOf = useSubjectOf()
  const pullRequests = useRepositoryPullRequestMap()
  useIndicatorClaim(APPLY_FIX_SURFACE)
  const applying = useSurfaceBusy(APPLY_FIX_SURFACE) !== null
  // another rewrite of the repository runs: the fix waits for it in the queue, its button shows what it waits for
  const held = branchesOfFix(component).find(branch => holds.has(branch))
  const reason =
    held !== undefined ? `A Safe Push of ${held} is running` : holdsPending ? formatPendingFact("patronus") : viewFailure === null ? null : formatViewFailure(viewFailure)
  const steps = component.steps.map(step => describeFixStep(step, subjectOf))
  const grouping = describeFixGrouping(component)
  const result = describeFixResult(component, branch => pullRequests[branch]?.state === "open")
  const apply = (
    <>
      <Button size="sm" disabled={applying || reason !== null} onClick={() => void applyConflictFix(component)} data-testid="apply-conflict-fix">
        {applying ? <Spinner data-icon="inline-start" /> : <WandSparklesIcon data-icon="inline-start" />}
        Apply Fix
      </Button>
      <LockWaitNotice surface={APPLY_FIX_SURFACE} />
    </>
  )
  return (
    // one step is the title; more are a list under "Fix: N steps"
    <FixCallout title={steps.length === 1 ? steps[0]! : `Fix in ${steps.length} steps`} state="found">
      {steps.length > 1 && (
        <HintList>
          {steps.map((step, index) => (
            <li key={index} data-testid="fix-step">
              {step}
            </li>
          ))}
        </HintList>
      )}
      {grouping.length > 0 && <p>{`${grouping.join(". ")}.`}</p>}
      {result !== "" && <p data-testid="fix-result">{result}</p>}
      <div className="flex flex-wrap items-center gap-2 pt-1">
        {reason === null ? (
          apply
        ) : (
          <Tooltip>
            <TooltipTrigger render={<span tabIndex={0} />}>{apply}</TooltipTrigger>
            <TooltipContent>{reason}</TooltipContent>
          </Tooltip>
        )}
        <span className="text-xs">{reason ?? "Undo is in the toast and in Activity"}</span>
      </div>
    </FixCallout>
  )
}

function LookAgain() {
  return (
    <Button size="xs" variant="outline" onClick={() => void requestConflictFix(true)}>
      <RotateCwIcon data-icon="inline-start" />
      Look Again
    </Button>
  )
}

function CopyConflictPrompt({ conflict, branchName, builtOn, owners }: ConflictFixPanelProps) {
  const project = useSelectedProject()
  const bd = useAgentCliCommand(project?.path ?? null)
  const { isCopied, copyToClipboard } = useCopyToClipboard()
  const copy = () =>
    copyToClipboard(
      buildConflictPrompt({
        repositoryPath: project?.path ?? "",
        branch: branchName,
        commit: { hash: conflict.commit.hash, subject: parseSubject(getSubjectLine(conflict.commit.message)).rest },
        builtOn: builtOn ?? "the baseline",
        files: conflict.conflictingFiles.map(file => file.file),
        explanation: formatMissingCommitsTitle(owners),
        bd: bd ?? null,
      }),
    )
  return (
    <Button size="xs" variant="outline" data-testid="copy-conflict-prompt" disabled={bd === undefined} onClick={() => void copy()}>
      {isCopied ? <CopyCheckIcon data-icon="inline-start" /> : <CopyIcon data-icon="inline-start" />}
      {isCopied ? "Copied" : "Copy Prompt for Agent"}
    </Button>
  )
}
