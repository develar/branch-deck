import { ArrowDownIcon, CheckIcon, CircleHelpIcon, PackageCheckIcon, TriangleAlertIcon } from "lucide-react"
import type { LucideProps } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { TransferProgress, UpdatePhase } from "@/lib/bindings"
import { describeLanding, shortLanding } from "@/lib/landed"
import { isMac } from "@/lib/platform"
import { formatRelativeTime } from "@/lib/time"
import { transferAmount } from "@/lib/transfer-progress"
import type { UpdateStateDescription } from "@/lib/update-state"
import { describeUpdateState, lastFetchTime, runningUpdateSummary, updatePhaseLabel } from "@/lib/update-state"
import type { BaselineStatus } from "@/stores/branch-sync"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useLanding } from "@/stores/landed"
import { useSelectedProject } from "@/stores/repository"
import { runUpdateProject, useFetchProgress, useUpdatePhase } from "@/stores/update-project"

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

/** The remote branch the update fetches, e.g. "origin/master"; the baseline name before the first check */
function upstreamName(status: BaselineStatus): string {
  return status.update?.upstream ?? status.baselineBranch
}

function useFetchTime(status: BaselineStatus): number | null {
  const ownFetchTime = useSelectedProject()?.lastFetchTime
  return lastFetchTime(status.update, ownFetchTime)
}

/** The icon props pass through, e.g. `data-icon` inside the button */
interface StateIconProps extends LucideProps {
  status: BaselineStatus
  /** The update brings in the user's own landed commits */
  landing: boolean
}

/**
 * The state icon of the button and of the footer line: a question mark for a failed check, the `info` package for a
 * clean update that brings in the user's own landed commits, a check for a clean one, a warning; none when up to date.
 */
function StateIcon({ status, landing, className, ...props }: StateIconProps) {
  const iconProps = { ...props, "data-testid": "update-state-icon" }
  if (status.update === null) {
    return <CircleHelpIcon {...iconProps} className={cn("text-muted-foreground", className)} />
  }
  const tone = describeStatus(status).tone
  if (landing && tone === "success") {
    return <PackageCheckIcon {...iconProps} className={cn("text-info", className)} data-landing />
  }
  switch (tone) {
    case "success":
      return <CheckIcon {...iconProps} className={cn("text-success", className)} />
    case "warning":
      return <TriangleAlertIcon {...iconProps} className={cn("text-warning", className)} />
    case "muted":
      return null
  }
}

interface RunningUpdate {
  phase: UpdatePhase | null
  fetch: TransferProgress | null
}

/**
 * Header button of Update Project (⌘T): fetches the baseline and rebases the branch onto it. Shows the state of the
 * last check, relative to the last fetch: muted when up to date, a check when it applies cleanly, a warning on a
 * conflict or when uncommitted changes block it. Never a count of upstream commits. Hidden for a local baseline.
 * While the fetch runs, the label has its percent and the tooltip the last progress line of git, as IDEA shows it.
 * While an update runs, the tooltip names the step, never the state of the last check.
 */
export function UpdateProjectButton() {
  const status = useRemoteBaselineStatus()
  const phase = useUpdatePhase()
  const fetch = useFetchProgress()
  if (status === null) {
    return null
  }
  return <UpdateProjectButtonContent status={status} phase={phase} fetch={fetch} />
}

function UpdateProjectButtonContent({ status, phase, fetch }: RunningUpdate & { status: BaselineStatus }) {
  const description = describeStatus(status)
  const fetchTime = useFetchTime(status)
  const landing = useLanding()
  const landingSentence = describeLanding(landing)
  const summary = landingSentence === null ? description.summary : `${description.summary} · ${landingSentence}`
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
            // the tooltip with the progress line stays reachable while the update runs
            focusableWhenDisabled
            aria-busy={phase !== null || undefined}
            aria-label={`Update Project: ${summary}`}
            data-testid="update-project-button"
            data-state={state}
            onClick={() => void runUpdateProject()}
          />
        )}
      >
        {phase === null ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {phase === null ? "Update" : updatePhaseLabel(phase, fetch)}
        {phase === null && <StateIcon status={status} landing={landing.length > 0} data-icon="inline-end" />}
      </TooltipTrigger>
      <TooltipContent data-testid="update-project-tooltip">
        <div className="flex flex-col gap-0.5">
          <span>{phase === null ? summary : runningUpdateSummary(phase, fetch, upstreamName(status))}</span>
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

/**
 * The update state in one line with an Update button, for the footer of the outgoing overview. Says "Incoming" and the
 * fetch time: below the push rows, a bare "origin/master: up to date" reads as nothing to push.
 */
export function UpdateStateLine() {
  const status = useRemoteBaselineStatus()
  const phase = useUpdatePhase()
  const fetch = useFetchProgress()
  if (status === null) {
    return null
  }
  return <UpdateStateLineContent status={status} phase={phase} fetch={fetch} />
}

/** While the fetch receives, the text has the amount and the rate; the percent is on the button */
function UpdateStateLineContent({ status, phase, fetch }: RunningUpdate & { status: BaselineStatus }) {
  const description = describeStatus(status)
  const fetchTime = useFetchTime(status)
  const transfer = phase === "fetching" ? transferAmount(fetch) : null
  const landed = useLanding()
  const landing = shortLanding(landed)
  return (
    <div className="flex items-center justify-between gap-3" data-testid="update-state-line">
      <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
        {transfer === null && <StateIcon status={status} landing={landed.length > 0} className="size-3 shrink-0" />}
        {/* wraps: the overview is at most `max-w-xl`, and a truncated state says nothing */}
        <span className="min-w-0">
          {"Incoming from "}
          <span className="text-foreground">{upstreamName(status)}</span>
          {transfer === null
            ? `: ${description.short}${landing === null ? "" : ` · ${landing}`} · ${fetchTime === null ? "not fetched yet" : `fetched ${formatRelativeTime(fetchTime)}`}`
            : `: ${transfer}`}
        </span>
      </span>
      <Button
        variant="ghost"
        size="xs"
        disabled={phase !== null}
        // full opacity while the update runs, as the header button
        focusableWhenDisabled
        aria-busy={phase !== null || undefined}
        data-testid="update-state-line-button"
        onClick={() => void runUpdateProject()}
      >
        {phase === null ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {phase === null ? "Update" : updatePhaseLabel(phase, fetch)}
      </Button>
    </div>
  )
}
