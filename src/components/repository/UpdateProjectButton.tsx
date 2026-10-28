import { cn } from "cn"
import { ArrowDownIcon, CheckIcon, CircleHelpIcon, CircleIcon, PackageCheckIcon, TriangleAlertIcon } from "lucide-react"
import type { LucideProps } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { ProgressLine } from "@/components/shared/ProgressLine"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useRunProgress } from "@/hooks/useRunProgress"
import { describeLanding, shortLanding } from "@/lib/landed"
import { isMac } from "@/lib/platform"
import { progressPercent } from "@/lib/update-progress"
import type { UpdateStateDescription, UpdateTone } from "@/lib/update-state"
import {
  describeAsFetched,
  describeUpdateState,
  fetchAndUpdateLine,
  lastFetchTime,
  runFooterWords,
  runLabel,
  runningStepSentence,
  runStepLines,
  stopWaitingUpdateItem,
  updateFooterWords,
} from "@/lib/update-state"
import type { BaselineStatus } from "@/stores/branch-sync"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useLanding } from "@/stores/landed"
import { useSelectedProjectPath } from "@/stores/repository"
import { useRepositoryStateField } from "@/stores/settings/repositoryState"
import type { RunningUpdate } from "@/stores/update-project"
import { runUpdateProject, useHeadWait, useShownUpdate } from "@/stores/update-project"

/**
 * The state of the check as of the last fetch (`fetchTime`). A check that failed (e.g. a rebase in progress) says why:
 * the update will say why it can't run.
 */
function describeStatus(status: BaselineStatus, fetchTime: number | null): UpdateStateDescription {
  if (status.update === null) {
    return { tone: "muted", summary: `Cannot check the update: ${status.checkError ?? "unknown error"}`, short: "not checked" }
  }
  return describeAsFetched(describeUpdateState(status.update.state, status.update.upstream, status.update.resolvedByWords), status.update.upstream, fetchTime)
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
  const ownFetchTime = useRepositoryStateField(useSelectedProjectPath(), "lastFetchTime")
  return lastFetchTime(status.update, ownFetchTime)
}

/** The icon props pass through, e.g. `data-icon` inside the button */
interface StateIconProps extends LucideProps {
  tone: UpdateTone
  /** The check failed: the update will say why it can't run */
  unknown: boolean
  /** The update brings in the user's own landed commits */
  landing: boolean
}

/**
 * The state icon of the button and of the footer line: a question mark for a failed check, the `info` package for a
 * clean update that brings in the user's own landed commits, a check for a clean one, a warning; none when up to date.
 */
function StateIcon({ tone, unknown, landing, className, ...props }: StateIconProps) {
  const iconProps = { ...props, "data-testid": "update-state-icon" }
  if (unknown) {
    return <CircleHelpIcon {...iconProps} className={cn("text-muted-foreground", className)} />
  }
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

/** "Fetched 5 minutes ago · fetch and update", the second line of the tooltip: it ages while it is shown */
function FetchAndUpdateLine({ fetchTime }: { fetchTime: number | null }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return fetchAndUpdateLine(fetchTime, now)
}

/** "rebases cleanly · fetched 5 minutes ago" after "Incoming from origin/master: ": it ages while it is shown */
function UpdateFooterWords({ description, fetchTime, landing }: { description: UpdateStateDescription; fetchTime: number | null; landing: string | null }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return updateFooterWords(description, fetchTime, landing, now)
}

/** "Updating 62%": the label of a running update, with the width of the widest one, so the header doesn't move per tick */
function RunLabel({ shown }: { shown: RunningUpdate }) {
  const progress = useRunProgress(shown)
  return (
    // the widest label as an invisible pseudo-element in the same cell: it holds the width, and is no text of the button
    <span
      className="grid before:invisible before:col-start-1 before:row-start-1 before:content-[attr(data-widest)]"
      data-widest={shown.run.kind === "fetch" ? "Fetching 99%" : "Updating 99%"}
    >
      <span className="col-start-1 row-start-1">{runLabel(shown.run, progress)}</span>
    </span>
  )
}

/** The bar of the whole run along the bottom edge of its button, a sibling of it: inside a button it has no role */
function RunProgressLine({ shown, upstream }: { shown: RunningUpdate; upstream: string }) {
  const progress = useRunProgress(shown)
  const percent = shown.endedAt === null ? progressPercent(progress) : 100
  return (
    <ProgressLine
      progress={progress}
      label={shown.run.kind === "fetch" ? "Fetch progress" : "Update progress"}
      valueText={`${runningStepSentence(shown.run, upstream)} ${percent}%`}
      speed="fast"
      className="pointer-events-none absolute inset-x-1.5 bottom-0 h-0.5"
      data-testid="update-progress"
      data-step={shown.run.step}
    />
  )
}

// on the first line of a step that wraps (a long progress line of git): 12 px in the 16 px line of `text-xs`
const STEP_ICON_CLASS = "mt-0.5 size-3 shrink-0"

/** The steps of a running update in its tooltip: those done, the running one, those still expected */
function RunSteps({ shown, upstream }: { shown: RunningUpdate; upstream: string }) {
  return (
    <ul className="flex flex-col gap-0.5" data-testid="update-steps">
      {runStepLines(shown.run, upstream).map(line => (
        <li key={line.step} className={cn("flex items-start gap-1.5", line.state === "next" && "opacity-70")} data-state={line.state}>
          {line.state === "done" && <CheckIcon className={STEP_ICON_CLASS} />}
          {line.state === "running" && <Spinner className={STEP_ICON_CLASS} />}
          {line.state === "next" && <CircleIcon className={STEP_ICON_CLASS} />}
          <span>{line.text}</span>
        </li>
      ))}
    </ul>
  )
}

interface ShownRun {
  /** The run of the repository, also a moment after its end; undefined when none */
  shown: RunningUpdate | undefined
}

/** The run of the shown update while it runs; a run that just ended shows only on its bar */
function activeRun(shown: RunningUpdate | undefined): RunningUpdate | undefined {
  return shown?.endedAt === null ? shown : undefined
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
  const shown = useShownUpdate()
  if (status === null) {
    return null
  }
  return <UpdateProjectButtonContent status={status} shown={shown} />
}

function UpdateProjectButtonContent({ status, shown }: ShownRun & { status: BaselineStatus }) {
  const running = activeRun(shown)
  // a regroup or a squash holds HEAD: the check and the move of an update wait for it in the queue ("Unstacking oauth…")
  const waitsFor = useHeadWait()
  const fetchTime = useFetchTime(status)
  const description = describeStatus(status, fetchTime)
  const landing = useLanding()
  const landingSentence = describeLanding(landing)
  const summary = landingSentence === null ? description.summary : `${description.summary} · ${landingSentence}`
  const state = status.update?.state.status ?? "unknown"
  const upstream = upstreamName(status)
  // the update waits for another holder of the repository lock: Stop Waiting beside the button, the tooltip says whom for
  const stopWaiting = stopWaitingUpdateItem(running?.run)
  return (
    <span className="relative inline-flex">
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              variant="ghost"
              size="sm"
              className={cn("tabular-nums", description.tone === "muted" && running === undefined && "text-muted-foreground")}
              disabled={running !== undefined}
              // the tooltip with the steps stays reachable while the update runs
              focusableWhenDisabled
              aria-busy={running !== undefined || undefined}
              // the bar says how far; a name that changed every tick would be read again and again
              aria-label={running === undefined ? `Update Project: ${summary}` : `Update Project: ${running.run.kind === "fetch" ? "fetching" : "updating"}`}
              data-testid="update-project-button"
              data-status={state}
              onClick={() => void runUpdateProject()}
            />
          }
        >
          {running === undefined ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
          {running === undefined ? "Update" : <RunLabel shown={running} />}
          {running === undefined && <StateIcon tone={description.tone} unknown={status.update === null} landing={landing.length > 0} data-icon="inline-end" />}
        </TooltipTrigger>
        <TooltipContent data-testid="update-project-tooltip">
          {running === undefined ? (
            <div className="flex flex-col gap-0.5">
              <span>{waitsFor ?? summary}</span>
              <span className="flex items-center gap-1.5 opacity-70">
                <FetchAndUpdateLine fetchTime={fetchTime} />
                <Kbd>{isMac ? "⌘T" : "Ctrl+T"}</Kbd>
              </span>
            </div>
          ) : (
            <RunSteps shown={running} upstream={upstream} />
          )}
        </TooltipContent>
      </Tooltip>
      {shown !== undefined && <RunProgressLine shown={shown} upstream={upstream} />}
      {stopWaiting !== null && <MenuItemButton item={stopWaiting} variant="outline" size="sm" className="ml-1" data-testid="stop-waiting" />}
    </span>
  )
}

/**
 * The update state in one line with an Update button, for the footer of the landing overview. Says "Incoming" and the
 * fetch time: below the push rows, a bare "origin/master: up to date" reads as nothing to push.
 */
export function UpdateStateLine() {
  const status = useRemoteBaselineStatus()
  const shown = useShownUpdate()
  if (status === null) {
    return null
  }
  return <UpdateStateLineContent status={status} shown={shown} />
}

/**
 * While a run is on, the text names its step, and the amount and the rate while the fetch receives; the percent is on
 * the button. No second bar: the header button with its bar shows above the overview.
 */
function UpdateStateLineContent({ status, shown }: ShownRun & { status: BaselineStatus }) {
  const running = activeRun(shown)
  const fetchTime = useFetchTime(status)
  const description = describeStatus(status, fetchTime)
  const runWords = running === undefined ? null : runFooterWords(running.run)
  const landed = useLanding()
  const landing = shortLanding(landed)
  return (
    <div className="flex items-center justify-between gap-3" data-testid="update-state-line">
      <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
        {runWords === null && <StateIcon tone={description.tone} unknown={status.update === null} landing={landed.length > 0} className="size-3 shrink-0" />}
        {/* wraps: the overview is at most `max-w-xl`, and a truncated state says nothing */}
        <span className="min-w-0 tabular-nums">
          {"Incoming from "}
          <span className="text-foreground">{upstreamName(status)}</span>
          {": "}
          {runWords ?? <UpdateFooterWords description={description} fetchTime={fetchTime} landing={landing} />}
        </span>
      </span>
      <Button
        variant="ghost"
        size="xs"
        className="tabular-nums"
        disabled={running !== undefined}
        // full opacity while the update runs, as the header button
        focusableWhenDisabled
        aria-busy={running !== undefined || undefined}
        data-testid="update-state-line-button"
        onClick={() => void runUpdateProject()}
      >
        {running === undefined ? <ArrowDownIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
        {running === undefined ? "Update" : <RunLabel shown={running} />}
      </Button>
    </div>
  )
}
