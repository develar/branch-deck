import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { PopoverSectionTitle } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { getGatedRobotActions } from "@/hooks/useBranchNextStep"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import type { PatronusRobot } from "@/lib/bindings"
import { formatFailedChecks } from "@/lib/check-counts"
import { FROM_MENU } from "@/lib/menu"
import { formatPushMode, openRobot, robotRingColor } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatCheckProgress, formatRobotProgress } from "@/lib/robot-eta"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { isFailingEntry } from "@/lib/robot-overview"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatWholeStack } from "@/lib/whole-stack"
import { useBranch } from "@/stores/branch-sync"
import { useOperations } from "@/stores/operations"
import { useRobotNeedsReview } from "@/stores/patronus"

interface RobotListProps {
  /** The section title; "Landing · 2" */
  title: string
  testId: string
  entries: readonly RobotOverviewEntry[]
  onReveal: (name: string) => void
}

export function RobotList({ title, testId, entries, onReveal }: RobotListProps) {
  if (entries.length === 0) {
    return null
  }
  return (
    <section aria-label={title} className="flex flex-col gap-1" data-testid={testId}>
      <PopoverSectionTitle>{title}</PopoverSectionTitle>
      <ul className="flex flex-col gap-1.5">
        {entries.map(entry => (
          <RobotRow key={entry.robot.id} entry={entry} onReveal={onReveal} />
        ))}
      </ul>
    </section>
  )
}

/**
 * A robot: its icon, the branch (a click shows its row), below it the mode (a link that opens the robot in Patronus),
 * the merge profile when an answer named it, and the time or the state ("Safe Push · Large Scale Change · 18m · ~1h 30m
 * left"), the progress bar of a running one; then the actions of the robot as icon buttons, named as in the branch menu
 * (`getGatedRobotActions`). Without `onReveal` the branch is left out: the surface is the branch (the hover of its node).
 */
export function RobotRow({ entry, onReveal }: { entry: RobotOverviewEntry; onReveal?: (name: string) => void }) {
  const { robot, chip, branchName } = entry
  const branch = useBranch(entry.stackRoot === undefined ? branchName : "")
  // the whole stack is named in a sentence as "the whole air stack", and a click shows its root
  const subject = entry.stackRoot === undefined ? branchName : formatWholeStack(entry.stackRoot)
  const reveal = entry.stackRoot ?? branchName
  const needsReview = useRobotNeedsReview(robot.id)
  // a restart or a cancel that runs shows in its button
  const operations = useOperations()
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
        {onReveal === undefined ? null : entry.hasRow ? (
          <Tooltip>
            <TooltipTrigger
              render={
                <button
                  type="button"
                  className="min-w-0 self-start truncate rounded-sm text-left text-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
                  onClick={() => onReveal(reveal)}
                />
              }
            >
              {branchName}
            </TooltipTrigger>
            <TooltipContent>{`Show ${reveal}`}</TooltipContent>
          </Tooltip>
        ) : (
          <span className="min-w-0 truncate text-sm">{branchName}</span>
        )}
        <span className="text-xs text-muted-foreground tabular-nums" data-testid="robots-row-state">
          <RobotModeLink robot={robot} mode={mode} />
          {" · "}
          {robot.profile?.name != null && `${robot.profile.name} · `}
          {active ? (
            <RunningTime entry={entry} />
          ) : (
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
        {getGatedRobotActions(robot, branch ?? null, needsReview, failing, operations).map(action => (
          <IconAction key={action.id} action={action} label={action.id === "open-robot" ? `Open ${mode} of ${subject} in Patronus` : `${action.label} of ${subject}`} />
        ))}
      </span>
    </li>
  )
}

/** The mode of the robot ("Safe Push", "Dry Run") as a link: a click opens the robot in Patronus */
function RobotModeLink({ robot, mode }: { robot: PatronusRobot; mode: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            className="rounded-sm outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
            data-testid="robots-row-mode"
            onClick={() => openRobot(robot)}
          />
        }
      >
        {mode}
      </TooltipTrigger>
      <TooltipContent>Open in Patronus</TooltipContent>
    </Tooltip>
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
      {failed !== null && (
        <span className="text-xs break-words text-destructive" data-testid="robots-row-failed">
          {failed}
        </span>
      )}
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
function IconAction({ action, label }: { action: RobotAction; label: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            disabled={action.disabled}
            // a disabled action says why on hover: Base UI keeps it focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn("text-muted-foreground", action.disabled && "opacity-50")}
            aria-label={label}
            aria-busy={action.running === true || undefined}
            onClick={() => action.onSelect(FROM_MENU)}
          />
        }
      >
        {/* a restart or a cancel that runs: the spinner in place of its icon */}
        {action.running === true ? <Spinner /> : action.icon && <action.icon />}
      </TooltipTrigger>
      <TooltipContent>{action.description === undefined ? action.label : `${action.label}: ${action.description}`}</TooltipContent>
    </Tooltip>
  )
}
