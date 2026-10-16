import { useState } from "react"
import { ShieldIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverSectionTitle, PopoverTitle, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { formatPushMode, robotRingColor } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { getRobotActions } from "@/lib/patronus-actions"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatFailedChecks } from "@/lib/check-counts"
import { formatCheckProgress, formatRobotProgress } from "@/lib/robot-eta"
import type { RobotIndicatorPart, RobotOverview, RobotOverviewEntry } from "@/lib/robot-overview"
import { formatRobotIndicatorLabel, getRobotIndicatorParts, isFailingEntry } from "@/lib/robot-overview"
import { statusChipTextColors } from "@/lib/status-chip"
import { useBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useRobotNeedsReview } from "@/stores/patronus"

const partColors: Readonly<Record<RobotIndicatorPart["kind"], string>> = {
  running: "text-foreground",
  failing: "text-destructive",
  failed: "text-destructive",
  needsReview: "text-warning",
}

/**
 * Header button with the Patronus robots of the repository's branches: a shield in the rotating ring and "2 running"
 * while robots run, "1 failing" (`destructive`) for a running one that will fail, "1 failed" (`destructive`) and "1
 * needs review" (`warning`) for the branches whose robot failed on the branch as it is, the parts together, compactly; hidden while nothing runs and nothing failed. Hover or click opens
 * the list: the running robots with their time and a progress bar, the failed ones, then the others that finished in the
 * last day. A branch name shows its row. It reads only the robots already listed.
 */
export function RobotsIndicator() {
  const overview = useRobotOverview()
  const parts = getRobotIndicatorParts(overview)
  const [open, setOpen] = useState(false)
  // nothing left to show: the next robot must not reopen the popover
  if (parts.length === 0 && open) {
    setOpen(false)
  }
  if (parts.length === 0) {
    return null
  }

  const running = overview.running.length > 0
  const anyFailed = parts.some(part => part.kind === "failed" || part.kind === "failing")
  const AttentionIcon = anyFailed ? patronusChipIcons.failed : patronusChipIcons.needsReview
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={<Button variant="ghost" size="sm" aria-label={formatRobotIndicatorLabel(parts)} data-testid="robots-indicator" />}
      >
        {running
          ? (
              // the shield in the ring of a running robot
              <span aria-hidden className="relative flex size-4 items-center justify-center">
                <RobotRing className="absolute inset-0 size-4 text-primary" />
                <ShieldIcon className="size-2" />
              </span>
            )
          : <AttentionIcon data-icon="inline-start" className={anyFailed ? "text-destructive" : "text-warning"} />}
        <span className="flex items-center gap-1 tabular-nums">
          {parts.map((part, index) => (
            <span key={part.kind} className={partColors[part.kind]} data-testid={`robots-${part.kind}`}>
              {index > 0 && <span className="text-muted-foreground">{"· "}</span>}
              {part.text}
            </span>
          ))}
        </span>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 gap-3 p-3" data-testid="robots-popover">
        <RobotsOverview overview={overview} onClose={() => setOpen(false)} />
      </PopoverContent>
    </Popover>
  )
}

/** The lists of the popover: running, failed on the branch as it is, the others of the last day */
function RobotsOverview({ overview, onClose }: { overview: RobotOverview, onClose: () => void }) {
  const reveal = (name: string) => {
    requestReveal({ branchName: name, commitIds: [] })
    onClose()
  }
  return (
    <>
      <PopoverTitle className="text-sm">Patronus robots</PopoverTitle>
      <RobotList title="Running" testId="robots-running" entries={overview.running} onReveal={reveal} />
      <RobotList title="Needs attention" testId="robots-attention" entries={overview.attention} onReveal={reveal} />
      <RobotList title="Last 24 hours" testId="robots-finished" entries={overview.finished} onReveal={reveal} />
    </>
  )
}

interface RobotListProps {
  title: string
  testId: string
  entries: readonly RobotOverviewEntry[]
  onReveal: (name: string) => void
}

function RobotList({ title, testId, entries, onReveal }: RobotListProps) {
  if (entries.length === 0) {
    return null
  }
  return (
    <section aria-label={title} className="flex flex-col gap-1" data-testid={testId}>
      <PopoverSectionTitle>{title}</PopoverSectionTitle>
      <ul className="flex flex-col gap-1.5">
        {entries.map(entry => <RobotRow key={entry.robot.id} entry={entry} onReveal={onReveal} />)}
      </ul>
    </section>
  )
}

/**
 * A robot: its icon, the branch (a click shows its row), below it the mode, the merge profile when an answer named it,
 * and the time or the state ("Safe Push · Large Scale Change · 18m · ~1h 30m left"), the progress bar of a running one;
 * then the actions of the robot as icon buttons, named as in the branch menu (`getRobotActions`)
 */
function RobotRow({ entry, onReveal }: { entry: RobotOverviewEntry, onReveal: (name: string) => void }) {
  const { robot, chip, branchName } = entry
  const branch = useBranch(branchName)
  const needsReview = useRobotNeedsReview(robot.id)
  const mode = formatPushMode(robot.pushMode)
  const Icon = patronusChipIcons[chip.kind]
  const active = chip.kind === "running"
  const failing = isFailingEntry(entry)
  return (
    <li className="flex items-start gap-2" data-testid="robots-row" data-branch-name={branchName} data-kind={chip.kind}>
      <span className="flex h-5 items-center">
        {active ? <RobotRing className={cn("size-3.5", robotRingColor(failing))} /> : <Icon className={cn("size-3.5", statusChipTextColors[chip.color])} />}
      </span>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        {entry.hasRow
          ? (
              <Tooltip>
                <TooltipTrigger
                  render={<button type="button" className="min-w-0 self-start truncate rounded-sm text-left text-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30" onClick={() => onReveal(branchName)} />}
                >
                  {branchName}
                </TooltipTrigger>
                <TooltipContent>{`Show ${branchName}`}</TooltipContent>
              </Tooltip>
            )
          : <span className="min-w-0 truncate text-sm">{branchName}</span>}
        <span className="text-xs text-muted-foreground tabular-nums" data-testid="robots-row-state">
          {`${mode} · `}
          {robot.profile?.name != null && `${robot.profile.name} · `}
          {active
            ? <RunningTime entry={entry} />
            : (
                <>
                  <span className={statusChipTextColors[chip.color]}>{chip.label}</span>
                  {robot.finishTime !== null && (
                    <>
                      {" · "}
                      <RelativeTime time={robot.finishTime * 1000} />
                    </>
                  )}
                </>
              )}
        </span>
        {active && <RunningProgress entry={entry} />}
      </span>
      <span className="flex shrink-0">
        {getRobotActions(robot, branch ?? null, needsReview).map(action => (
          <IconAction key={action.id} action={action} label={action.id === "open-robot" ? `Open ${mode} of ${branchName} in Patronus` : `${action.label} of ${branchName}`} />
        ))}
      </span>
    </li>
  )
}

/** "18m · ~22m left", "18m · Failing" (no use waiting for it), "Canceling"; it ticks, so it is its own component */
function RunningTime({ entry }: { entry: RobotOverviewEntry }) {
  const timing = useRobotTiming(entry.robot, true)
  if (entry.robot.status === "canceling") {
    return entry.chip.label
  }
  const { elapsed, eta } = formatRobotProgress(timing)
  if (isFailingEntry(entry)) {
    return (
      <>
        <span className="text-foreground">{elapsed}</span>
        {" · "}
        <span className="text-destructive">{entry.chip.label}</span>
      </>
    )
  }
  return (
    <>
      <span className="text-foreground">{elapsed}</span>
      {eta !== null && ` · ${eta}`}
    </>
  )
}

/**
 * The share of the usual time, below the time; while the list is open the details of the robot are asked (as its
 * popover asks them): the checks that failed already ("Failed: Fast Tests", `destructive`), then the check the robot
 * waits for and the queued checks by the estimate of the CI
 */
function RunningProgress({ entry }: { entry: RobotOverviewEntry }) {
  const timing = useRobotTiming(entry.robot, true)
  const details = usePatronusRobotDetails(entry.robot.id, entry.robot.status, true)
  const checks = details?.progress ?? null
  const failed = details?.checks == null ? null : formatFailedChecks(details.checks)
  return (
    <>
      <RobotProgressBar progress={entry.robot.status === "canceling" ? null : timing.progress} failing={isFailingEntry(entry)} className="mt-0.5 h-1 rounded-sm" />
      {failed !== null && <span className="text-xs break-words text-destructive" data-testid="robots-row-failed">{failed}</span>}
      {checks !== null && (
        <span className="text-xs text-muted-foreground tabular-nums" data-testid="robots-row-checks">
          {formatCheckProgress(checks, { remaining: false })}
          {checks.probablyHanging && <span className="text-warning">{" · probably hanging"}</span>}
        </span>
      )}
    </>
  )
}

/** An action of the robot as an icon button; its name on hover, a disabled one says why. `label` names the branch too. */
function IconAction({ action, label }: { action: RobotAction, label: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant="ghost"
            size="icon-xs"
            disabled={action.disabled}
            // a disabled action says why on hover: Base UI keeps it focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn("text-muted-foreground", action.disabled && "opacity-50")}
            aria-label={label}
            onClick={action.onSelect}
          />
        )}
      >
        {action.icon && <action.icon />}
      </TooltipTrigger>
      <TooltipContent>{action.description === undefined ? action.label : `${action.label}: ${action.description}`}</TooltipContent>
    </Tooltip>
  )
}
