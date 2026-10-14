import { ArrowDownIcon, CheckIcon, CircleHelpIcon, PackageCheckIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { TransferProgress, UpdatePhase } from "@/lib/bindings"
import { describeLanding, shortLanding } from "@/lib/landed"
import { isMac } from "@/lib/platform"
import { transferAmount } from "@/lib/transfer-progress"
import type { UpdateStateDescription, UpdateTone } from "@/lib/update-state"
import {
  describeAsFetched,
  describeUpdateState,
  fetchAndUpdateLine,
  lastFetchTime,
  runningUpdateSummary,
  updateFooterWords,
  updatePhaseLabel,
} from "@/lib/update-state"
import type { BaselineStatus } from "@/stores/branch-sync"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useLanding } from "@/stores/landed"
import { useSelectedProject } from "@/stores/repository"
import { runUpdateProject, useFetchProgress, useUpdatePhase } from "@/stores/update-project"

/**
 * The state of the check as of the last fetch (`fetchTime`). A check that failed (e.g. a rebase in progress) says why:
 * the update will say why it can't run.
 */
function describeStatus(status: BaselineStatus, fetchTime: number | null): UpdateStateDescription {
  if (status.update === null) {
    return { tone: "muted", summary: `Can't check the update: ${status.checkError ?? "unknown error"}`, short: "not checked" }
  }
  return describeAsFetched(describeUpdateState(status.update.state, status.update.upstream), status.update.upstream, fetchTime)
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

/** The state icon; a clean update that brings in the user's own landed commits shows the `info` package */
function StateIcon({ tone, unknown, landing }: { tone: UpdateTone, unknown: boolean, landing: boolean }) {
  if (unknown) {
    return <CircleHelpIcon data-icon="inline-end" className="text-muted-foreground" data-testid="update-state-icon" />
  }
  if (landing && tone === "success") {
    return <PackageCheckIcon data-icon="inline-end" className="text-info" data-testid="update-state-icon" data-landing />
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

interface RunningUpdate {
  phase: UpdatePhase | null
  fetch: TransferProgress | null
}

/**
 * Header button of Update Project (⌘T): fetches the baseline and rebases the branch onto it. Shows the state of the
 * last check, relative to the last fetch: muted when up to date, a check when it applies cleanly, a warning on a
 * conflict or when uncommitted changes block it. Never a count of upstream commits. Hidden for a local baseline.
 * While the fetch runs, the label has its percent and the tooltip the last progress line of git, as IDEA shows it.
 * While an update runs, the tooltip names only the step: never the state of the last check, nor "fetch and update" of a
 * button that does nothing then. Before the first fetch it is muted, "origin/master not fetched yet".
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
  const fetchTime = useFetchTime(status)
  const description = describeStatus(status, fetchTime)
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
        {phase === null && <StateIcon tone={description.tone} unknown={status.update === null} landing={landing.length > 0} />}
      </TooltipTrigger>
      <TooltipContent data-testid="update-project-tooltip">
        {phase === null
          ? (
              <div className="flex flex-col gap-0.5">
                <span>{summary}</span>
                <span className="flex items-center gap-1.5 opacity-70">
                  {fetchAndUpdateLine(fetchTime)}
                  <Kbd>{isMac ? "⌘T" : "Ctrl+T"}</Kbd>
                </span>
              </div>
            )
          : <span>{runningUpdateSummary(phase, fetch, upstreamName(status))}</span>}
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
  const fetchTime = useFetchTime(status)
  const description = describeStatus(status, fetchTime)
  const transfer = phase === "fetching" ? transferAmount(fetch) : null
  const landing = shortLanding(useLanding())
  return (
    <div className="flex items-center justify-between gap-3" data-testid="update-state-line">
      <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
        {transfer === null && description.tone === "success" && <CheckIcon className="size-3 shrink-0 text-success" />}
        {transfer === null && description.tone === "warning" && <TriangleAlertIcon className="size-3 shrink-0 text-warning" />}
        <span className="truncate">
          {"Incoming from "}
          <span className="text-foreground">{upstreamName(status)}</span>
          {`: ${transfer ?? updateFooterWords(description, fetchTime, landing)}`}
        </span>
      </span>
      <Button variant="ghost" size="xs" disabled={phase !== null} data-testid="update-state-line-button" onClick={() => void runUpdateProject()}>
        {phase === null ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {phase === null ? "Update" : updatePhaseLabel(phase, fetch)}
      </Button>
    </div>
  )
}
