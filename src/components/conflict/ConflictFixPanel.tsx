import type { ReactNode } from "react"
import { ArrowDownIcon, CopyCheckIcon, CopyIcon, RotateCwIcon, WandSparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotActionButton } from "@/components/branch/PatronusChip"
import { HintList } from "@/components/conflict/ConflictResolutionHint"
import { Callout } from "@/components/shared/Callout"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { WidestLabel } from "@/components/shared/WidestLabel"
import { useAgentCliCommand } from "@/hooks/useAgentCliCommand"
import { getRobotItems, useRobotParts } from "@/hooks/useBranchNextStep"
import { useCopyToClipboard } from "@/hooks/useCopyToClipboard"
import type { FixComponent, MergeConflictInfo } from "@/lib/bindings"
import { componentFixing, describeFixGrouping, describeFixHold, describeFixResult, describeFixStep } from "@/lib/conflict-fix"
import { buildConflictPrompt } from "@/lib/conflict-fix-prompt"
import { formatMissingCommitsTitle } from "@/lib/merge-conflict"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import { formatPendingFact, formatViewFailure } from "@/presenters/next-step"
import { requestReveal } from "@/stores/commit-move"
import { APPLY_FIX_SURFACE, applyConflictFix, requestConflictFix, useConflictFix, useHolderHold, useSubjectOf } from "@/stores/conflict-fix"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/indicators"
import { usePatronusPending, usePatronusViewFailure, useRepositoryRobots } from "@/stores/patronus"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"
import { useSelectedProject } from "@/stores/repository"
import { runUpdateProjectOf } from "@/stores/update-project"

export interface ConflictFixPanelProps {
  conflict: MergeConflictInfo
  branchName: string
  builtOn: string | null
  owners: MissingCommitOwners
}

/**
 * The fix of a sync conflict (openspec/specs/branch/conflict-fix/spec.md), a callout below the explanation: looked for when the
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

/** The robot actions beside a hold of a branch of a fix, by id: of a running robot that lands it the robot alone (its cancel is on its row), of a failing one its restart and cancel too */
const LANDS_ACTIONS: ReadonlySet<string> = new Set(["open-robot"])
const FAILING_ACTIONS: ReadonlySet<string> = new Set(["restart-robot", "push-and-restart-robot", "cancel-robot", "open-robot"])

function FixSteps({ component }: { component: FixComponent }) {
  // what the robots hold is known only once Patronus answered with its view: the offer waits for it, never on "none"
  const holdsPending = usePatronusPending()
  const viewFailure = usePatronusViewFailure()
  const subjectOf = useSubjectOf()
  const pullRequests = useRepositoryPullRequestMap()
  // the hold of a branch that has what the fix brings: a robot that lands it makes the conflict go without the fix
  const holderHold = useHolderHold(component)
  const robots = useRepositoryRobots()
  const robot = holderHold === null ? undefined : robots?.[holderHold.hold.by.robotId]
  const robotParts = useRobotParts(robot)
  const project = useSelectedProject()
  useIndicatorClaim(APPLY_FIX_SURFACE)
  const applying = useSurfaceBusy(APPLY_FIX_SURFACE) !== null
  const reason = holdsPending ? formatPendingFact("patronus") : viewFailure === null ? null : formatViewFailure(viewFailure)
  const steps = component.steps.map(step => describeFixStep(step, subjectOf))
  const grouping = describeFixGrouping(component)
  const result = describeFixResult(component, branch => pullRequests[branch]?.state === "open")
  const kind = holderHold?.hold.kind
  const words = holderHold === null ? null : describeFixHold(holderHold.hold, holderHold.holder, component.fixed)

  if (kind === "landed" && words !== null) {
    return (
      <FixCallout title={words.title ?? ""} state="found">
        <p data-testid="fix-hold">{words.detail}</p>
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <Button size="sm" disabled={project === null} onClick={() => project !== null && void runUpdateProjectOf(project.path)} data-testid="fix-update-project">
            <ArrowDownIcon data-icon="inline-start" />
            Update Project
          </Button>
        </div>
      </FixCallout>
    )
  }

  // waiting is the way while a robot lands what the fix brings: the fix goes below it, as the other choice
  const waits = kind === "lands"
  const robotActions = robot === undefined ? [] : getRobotItems(robot, robotParts).filter(it => (waits ? LANDS_ACTIONS : FAILING_ACTIONS).has(it.id))
  const apply = (
    <>
      <Button
        // the other choice beside a wait is as small as the robot's buttons
        size={waits ? "xs" : "sm"}
        variant={waits ? "outline" : "default"}
        disabled={applying || reason !== null}
        // busy, it keeps the focus, which Stop Waiting of its wait gives back to it (`LockWaitNotice`)
        focusableWhenDisabled={applying}
        aria-busy={applying || undefined}
        onClick={() => void applyConflictFix(component)}
        data-testid="apply-conflict-fix"
      >
        {applying ? <Spinner data-icon="inline-start" /> : <WandSparklesIcon data-icon="inline-start" />}
        Apply Fix
      </Button>
      <LockWaitNotice surface={APPLY_FIX_SURFACE} />
    </>
  )
  const stepsTitle = steps.length === 1 ? steps[0]! : `Fix in ${steps.length} steps`
  const stepList = steps.length > 1 && (
    <HintList>
      {steps.map((step, index) => (
        <li key={index} data-testid="fix-step">
          {step}
        </li>
      ))}
    </HintList>
  )
  return (
    // one step is the title; more are a list under "Fix in N steps"; a wait for a robot is the title above the fix
    <FixCallout title={waits && words?.title != null ? words.title : stepsTitle} state="found">
      {words !== null && <p data-testid="fix-hold">{words.detail}</p>}
      {waits && <p className="font-medium">{`Or fix it now: ${stepsTitle}`}</p>}
      {stepList}
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
        {robotActions.map(action => (
          <RobotActionButton key={action.id} action={action} />
        ))}
        {waits && holderHold !== null && (
          <Button size="xs" variant="outline" onClick={() => requestReveal({ branchName: holderHold.hold.by.branch, commitIds: [] })}>
            {`Show ${holderHold.hold.by.branch}`}
          </Button>
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
        commit: { hash: conflict.commit.hash, subject: conflict.commit.subjectParts.rest },
        builtOn: builtOn ?? "the baseline",
        files: conflict.conflictingFiles.map(file => file.file),
        explanation: formatMissingCommitsTitle(owners),
        bd: bd ?? null,
      }),
    )
  return (
    <Button size="xs" variant="outline" data-testid="copy-conflict-prompt" disabled={bd === undefined} onClick={() => void copy()}>
      {isCopied ? <CopyCheckIcon data-icon="inline-start" /> : <CopyIcon data-icon="inline-start" />}
      <WidestLabel widest={COPY_AGENT_PROMPT}>{isCopied ? "Copied" : COPY_AGENT_PROMPT}</WidestLabel>
    </Button>
  )
}

/** The words of the copy of the prompt, whose "Copied" holds their width */
const COPY_AGENT_PROMPT = "Copy Prompt for Agent"
