import { useState } from "react"
import type { ReactNode } from "react"
import { ExternalLinkIcon, ShieldIcon, ShieldOffIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { canCancelRobot, formatPushMode, openRobot } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatRobotProgress } from "@/lib/robot-eta"
import type { RobotIndicatorPart, RobotOverview, RobotOverviewEntry } from "@/lib/robot-overview"
import { formatRobotIndicatorLabel, getRobotIndicatorParts } from "@/lib/robot-overview"
import { statusChipTextColors } from "@/lib/status-chip"
import { requestReveal } from "@/stores/commit-move"
import { cancelPatronusRobot } from "@/stores/patronus"

const partColors: Readonly<Record<RobotIndicatorPart["kind"], string>> = {
  running: "text-foreground",
  failed: "text-destructive",
  needsReview: "text-warning",
}

/**
 * Header button with the Patronus robots of the repository's branches: a shield in the rotating ring and "2 running"
 * while robots run, "1 failed" (`destructive`) and "1 needs review" (`warning`) for the branches whose robot failed on
 * the branch as it is, the parts together, compactly; hidden while nothing runs and nothing failed. Hover or click opens
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
  const anyFailed = parts.some(part => part.kind === "failed")
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
      <h3 className="text-xs text-muted-foreground">{title}</h3>
      <ul className="flex flex-col gap-1.5">
        {entries.map(entry => <RobotRow key={entry.robot.id} entry={entry} onReveal={onReveal} />)}
      </ul>
    </section>
  )
}

/**
 * A robot: its icon, the branch (a click shows its row), below it the mode and the time or the state, the progress bar
 * of a running one; then Cancel and "Open in Patronus"
 */
function RobotRow({ entry, onReveal }: { entry: RobotOverviewEntry, onReveal: (name: string) => void }) {
  const { robot, chip, branchName } = entry
  const mode = formatPushMode(robot.pushMode)
  const Icon = patronusChipIcons[chip.kind]
  const active = chip.kind === "running"
  return (
    <li className="flex items-start gap-2" data-testid="robots-row" data-branch-name={branchName} data-kind={chip.kind}>
      <span className="flex h-5 items-center">
        {active ? <RobotRing className="size-3.5 text-primary" /> : <Icon className={cn("size-3.5", statusChipTextColors[chip.color])} />}
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
        {canCancelRobot(robot) && (
          <IconAction label={`Cancel ${mode} of ${branchName}`} tooltip={`Cancel ${mode}`} onClick={() => void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`)}>
            <ShieldOffIcon />
          </IconAction>
        )}
        <IconAction label={`Open ${mode} of ${branchName} in Patronus`} tooltip="Open in Patronus" onClick={() => openRobot(robot)}>
          <ExternalLinkIcon />
        </IconAction>
      </span>
    </li>
  )
}

/** "18m · ~22m left", "Canceling"; it ticks, so it is its own component */
function RunningTime({ entry }: { entry: RobotOverviewEntry }) {
  const timing = useRobotTiming(entry.robot)
  if (entry.robot.status === "canceling") {
    return entry.chip.label
  }
  const { elapsed, eta } = formatRobotProgress(timing)
  return (
    <>
      <span className="text-foreground">{elapsed}</span>
      {eta !== null && ` · ${eta}`}
    </>
  )
}

/** The share of the usual time, below the time */
function RunningProgress({ entry }: { entry: RobotOverviewEntry }) {
  const timing = useRobotTiming(entry.robot)
  return <RobotProgressBar progress={entry.robot.status === "canceling" ? null : timing.progress} className="mt-0.5 h-1 rounded-sm" />
}

function IconAction({ label, tooltip, onClick, children }: { label: string, tooltip: string, onClick: () => void, children: ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={label} onClick={onClick} />}>
        {children}
      </TooltipTrigger>
      <TooltipContent>{tooltip}</TooltipContent>
    </Tooltip>
  )
}
