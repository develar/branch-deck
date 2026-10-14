import { ArrowDownIcon, CheckIcon, CircleHelpIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { UpdatePhase } from "@/lib/bindings"
import { isMac } from "@/lib/platform"
import { formatRelativeTime } from "@/lib/time"
import type { UpdateStateDescription, UpdateTone } from "@/lib/update-state"
import { describeUpdateState, lastFetchTime } from "@/lib/update-state"
import type { BaselineStatus } from "@/stores/branch-sync"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useSelectedProject } from "@/stores/repository"
import { runUpdateProject, useUpdatePhase } from "@/stores/update-project"

const PHASE_LABELS: Record<UpdatePhase, string> = {
  fetching: "Fetching…",
  checking: "Checking…",
  updating: "Updating…",
}

/** The state of a check that failed (e.g. a rebase in progress): the update will say why it can't run */
function describeStatus(status: BaselineStatus): UpdateStateDescription {
  if (status.update === null) {
    return { tone: "muted", summary: `Can't check the update: ${status.checkError ?? "unknown error"}`, short: "not checked" }
  }
  return describeUpdateState(status.update.state, status.update.upstream)
}

/** The baseline status if there is something to update from: null for a local baseline or before the first sync */
function useRemoteBaselineStatus(): BaselineStatus | null {
  const status = useBaselineStatus()
  return status === null || (status.update === null && status.checkError === null) ? null : status
}

function useFetchTime(status: BaselineStatus): number | null {
  const ownFetchTime = useSelectedProject()?.lastFetchTime
  return lastFetchTime(status.update, ownFetchTime)
}

function StateIcon({ tone, unknown }: { tone: UpdateTone, unknown: boolean }) {
  if (unknown) {
    return <CircleHelpIcon data-icon="inline-end" className="text-muted-foreground" data-testid="update-state-icon" />
  }
  switch (tone) {
    case "success":
      return <CheckIcon data-icon="inline-end" className="text-success" data-testid="update-state-icon" />
    case "warning":
      return <TriangleAlertIcon data-icon="inline-end" className="text-warning" data-testid="update-state-icon" />
    case "muted":
      return null
  }
}

/**
 * Header button of Update Project (⌘T): fetches the baseline and rebases the branch onto it. Shows the state of the
 * last check, relative to the last fetch: muted when up to date, a check when it applies cleanly, a warning on a
 * conflict or when uncommitted changes block it. Never a count of upstream commits. Hidden for a local baseline.
 */
export function UpdateProjectButton() {
  const status = useRemoteBaselineStatus()
  const phase = useUpdatePhase()
  if (status === null) {
    return null
  }
  return <UpdateProjectButtonContent status={status} phase={phase} />
}

function UpdateProjectButtonContent({ status, phase }: { status: BaselineStatus, phase: UpdatePhase | null }) {
  const description = describeStatus(status)
  const fetchTime = useFetchTime(status)
  const state = status.update?.state.status ?? "unknown"
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant="ghost"
            size="sm"
            className={cn(description.tone === "muted" && phase === null && "text-muted-foreground")}
            disabled={phase !== null}
            aria-busy={phase !== null || undefined}
            aria-label={`Update Project: ${description.summary}`}
            data-testid="update-project-button"
            data-state={state}
            onClick={() => void runUpdateProject()}
          />
        )}
      >
        {phase === null ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {phase === null ? "Update" : PHASE_LABELS[phase]}
        {phase === null && <StateIcon tone={description.tone} unknown={status.update === null} />}
      </TooltipTrigger>
      <TooltipContent data-testid="update-project-tooltip">
        <div className="flex flex-col gap-0.5">
          <span>{description.summary}</span>
          <span className="flex items-center gap-1.5 opacity-70">
            {fetchTime === null ? "Not fetched yet" : `Fetched ${formatRelativeTime(fetchTime)}`}
            {" · fetch and update"}
            <Kbd>{isMac ? "⌘T" : "Ctrl+T"}</Kbd>
          </span>
        </div>
      </TooltipContent>
    </Tooltip>
  )
}

/** The update state in one line with an Update button, for the footer of the outgoing overview */
export function UpdateStateLine() {
  const status = useRemoteBaselineStatus()
  const phase = useUpdatePhase()
  if (status === null) {
    return null
  }
  const description = describeStatus(status)
  return (
    <div className="flex items-center justify-between gap-3" data-testid="update-state-line">
      <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
        {description.tone === "success" && <CheckIcon className="size-3 shrink-0 text-success" />}
        {description.tone === "warning" && <TriangleAlertIcon className="size-3 shrink-0 text-warning" />}
        <span className="truncate">
          <span className="text-foreground">{status.update?.upstream ?? status.baselineBranch}</span>
          {`: ${description.short}`}
        </span>
      </span>
      <Button variant="ghost" size="xs" disabled={phase !== null} data-testid="update-state-line-button" onClick={() => void runUpdateProject()}>
        {phase === null ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {phase === null ? "Update" : PHASE_LABELS[phase]}
      </Button>
    </div>
  )
}
