import { Fragment, useEffect, useState } from "react"
import type { ReactNode } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { cn } from "cn"
import {
  ArrowDownIcon,
  ChevronRightIcon,
  ExternalLinkIcon,
  GitPullRequestIcon,
  OctagonXIcon,
  PackageCheckIcon,
  PenLineIcon,
  ShieldCheckIcon,
  TriangleAlertIcon,
  UploadIcon,
  UserCheckIcon,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { PopoverSectionTitle } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { DroppedCommitList } from "@/components/branch/DroppedCommitList"
import { LandedBranchRow } from "@/components/repository/LandedCard"
import { RelativeTime } from "@/components/shared/RelativeTime"
import type { LandedBranchLinks } from "@/lib/activity-window"
import { notifyError } from "@/lib/errors"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { ActivityDetails, ActivityEntry } from "@/stores/activity"
import { groupActivityByDay, isUndoOpen } from "@/stores/activity"

type LandedDetails = Extract<ActivityDetails, { type: "landed" }>

/** What the log acts with: the main window owns the log, the window asks it (`@/lib/activity-window-link`) */
export interface ActivityLogActions {
  /** The branches of the table: their names are buttons that show the row */
  readonly branches: ReadonlySet<string>
  /** The commits of the undismissed landed report */
  readonly undismissed: ReadonlySet<string>
  /** The links of the branches of the landed reports */
  readonly links: Readonly<Record<string, LandedBranchLinks>>
  readonly onReveal: (name: string) => void
  readonly onUndo: (entry: ActivityEntry) => void
  readonly onDismiss: () => void
}

/**
 * The log, newest first by day: what Branch Deck did, each entry with its icon, title, description and age. A branch
 * name that is a row of the table shows it; a landed report expands to its branches, a force push lists the remote
 * commits it dropped, a robot opens in Patronus, a rewrite has its Undo while it lasts. `openedAt` dates the days.
 */
export function ActivityLog({ entries, openedAt, actions }: { entries: readonly ActivityEntry[]; openedAt: number; actions: ActivityLogActions }) {
  const days = groupActivityByDay(entries, openedAt)
  return days.map(day => (
    <section key={day.start} aria-label={day.label} data-testid="activity-day">
      <PopoverSectionTitle className="sticky top-0 z-10 bg-card px-4 pt-2 pb-1">{day.label}</PopoverSectionTitle>
      <ul className="flex flex-col">
        {day.entries.map(entry => (
          <ActivityRow key={entry.id} entry={entry} actions={actions} />
        ))}
      </ul>
    </section>
  ))
}

function ActivityRow({ entry, actions }: { entry: ActivityEntry; actions: ActivityLogActions }) {
  const { branches, onReveal } = actions
  const details = entry.details
  return (
    <li className="flex gap-2.5 px-4 py-2" data-testid="activity-entry" data-kind={entry.kind}>
      <EntryIcon entry={entry} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex items-baseline gap-2">
          <span className="min-w-0 flex-1 text-sm break-words" data-testid="activity-title">
            <BranchLinks text={entry.title} branches={branches} onReveal={onReveal} />
          </span>
          <span className="shrink-0 text-xs text-muted-foreground">
            <RelativeTime time={entry.time} />
          </span>
        </div>
        {entry.description !== undefined && (
          <p className="text-xs break-words text-muted-foreground" data-testid="activity-description">
            <BranchLinks text={entry.description} branches={branches} onReveal={onReveal} />
          </p>
        )}
        {entry.undo !== undefined && <UndoAction entry={entry} onUndo={actions.onUndo} />}
        {details?.type === "robot" && (
          <OpenLinkAction
            label="Open in Patronus"
            testId="activity-open-robot"
            onOpen={() => void openUrl(details.url).catch((error: unknown) => notifyError("Cannot open the Patronus robot", error))}
          />
        )}
        {details?.type === "pullRequest" && <OpenLinkAction label="Open Pull Request" testId="activity-open-pull-request" onOpen={() => openPullRequestUrl(details.url)} />}
        {details?.type === "dropped" && (
          <div className="pt-1">
            <DroppedCommitList dropped={details} />
          </div>
        )}
        {details?.type === "landed" && <LandedReport details={details} actions={actions} />}
      </div>
    </li>
  )
}

const kindIcons: Readonly<Record<ActivityEntry["kind"], typeof UploadIcon>> = {
  push: UploadIcon,
  update: ArrowDownIcon,
  landed: PackageCheckIcon,
  patronus: ShieldCheckIcon,
  rewrite: PenLineIcon,
  review: UserCheckIcon,
  error: OctagonXIcon,
}

/** The kind of the entry, in the hue of its outcome */
function EntryIcon({ entry }: { entry: ActivityEntry }) {
  const className = "mt-0.5 size-4 shrink-0"
  if (entry.kind === "error") {
    return <OctagonXIcon aria-hidden className={cn(className, "text-destructive")} />
  }
  if (entry.warning === true) {
    return <TriangleAlertIcon aria-hidden className={cn(className, "text-warning")} />
  }
  if (entry.details?.type === "landed") {
    return <PackageCheckIcon aria-hidden className={cn(className, "text-info")} />
  }
  const Icon = kindIcons[entry.kind]
  return <Icon aria-hidden className={cn(className, "text-muted-foreground")} />
}

/** The words of a branch name of the table are a button that shows its row; other words stay text */
function BranchLinks({ text, branches, onReveal }: { text: string; branches: ReadonlySet<string>; onReveal: (name: string) => void }) {
  const parts: ReactNode[] = text.split(/(\s+)/).map((word, index) => {
    // the name without the punctuation around it: "(feature-auth)", "feature-auth:", "feature-auth,"
    const match = /^([("'“]*)(.+?)([)"'”,.:;]*)$/.exec(word)
    const name = match?.[2]
    if (match === null || name === undefined || !branches.has(name)) {
      return word
    }
    return (
      <Fragment key={index}>
        {match[1]}
        <button
          type="button"
          className="rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
          title={`Show ${name}`}
          data-testid="activity-branch"
          onClick={() => onReveal(name)}
        >
          {name}
        </button>
        {match[3]}
      </Fragment>
    )
  })
  return parts
}

/** Until the Undo window passes; false when it can't end (no window, used) */
function useCanUndo(entry: ActivityEntry): boolean {
  const until = entry.undo?.until ?? 0
  const [now, setNow] = useState(Date.now)
  useEffect(() => {
    const left = until - Date.now()
    if (left <= 0) {
      return
    }
    const timer = setTimeout(() => setNow(Date.now()), left)
    return () => clearTimeout(timer)
  }, [until])
  return isUndoOpen(entry, now)
}

/** "Undo" while the rewrite can be undone, as from its toast; "undone" after it was */
function UndoAction({ entry, onUndo }: { entry: ActivityEntry; onUndo: (entry: ActivityEntry) => void }) {
  const canUndo = useCanUndo(entry)
  if (entry.undo?.undone === true) {
    return (
      <span className="text-xs text-muted-foreground" data-testid="activity-undone">
        undone
      </span>
    )
  }
  if (!canUndo) {
    return null
  }
  return (
    <div className="-ml-2 flex">
      <Button size="xs" variant="ghost" data-testid="activity-undo" onClick={() => onUndo(entry)}>
        Undo
      </Button>
    </div>
  )
}

/** Opens the object of an entry in the browser: a robot in Patronus, a pull request on GitHub */
function OpenLinkAction({ label, testId, onOpen }: { label: string; testId: string; onOpen: () => void }) {
  return (
    <div className="-ml-2 flex">
      <Button size="xs" variant="ghost" data-testid={testId} onClick={onOpen}>
        <ExternalLinkIcon data-icon="inline-start" />
        {label}
      </Button>
    </div>
  )
}

/**
 * The report of a landed entry, collapsed: the rows of the island, each expanding to its commits, with the links of its
 * pull request and its robot. While its commits are in the undismissed report (the chips and "New" marks show, the
 * island may be hidden), Dismiss clears them.
 */
function LandedReport({ details, actions }: { details: LandedDetails; actions: ActivityLogActions }) {
  const [expanded, setExpanded] = useState(false)
  const undismissed = details.report.some(branch => branch.commits.some(commit => actions.undismissed.has(commit.id)))
  const branchCount = details.report.length
  return (
    <div className="flex flex-col gap-1">
      <div className="-ml-2 flex items-center gap-1">
        <Button size="xs" variant="ghost" aria-expanded={expanded} data-testid="activity-landed-toggle" onClick={() => setExpanded(!expanded)}>
          <ChevronRightIcon data-icon="inline-start" className={cn("transition-transform duration-200", expanded && "rotate-90")} />
          {`What landed in ${details.upstream}`}
        </Button>
        {undismissed && (
          <Button size="xs" variant="ghost" data-testid="activity-landed-dismiss" title="Clear the landed marks of the branches" onClick={actions.onDismiss}>
            Dismiss
          </Button>
        )}
      </div>
      {expanded && (
        <ul
          className="divide-y divide-border overflow-hidden rounded-lg border border-border"
          aria-label={branchCount === 1 ? "1 branch" : `${branchCount} branches`}
          data-testid="activity-landed-report"
        >
          {details.report.map(branch => (
            <LandedBranchRow key={branch.name ?? ""} branch={branch} showCommitter compact renderLinks={name => <LandedLinks name={name} links={actions.links[name]} />} />
          ))}
        </ul>
      )}
    </div>
  )
}

/** The pull request and the robot of a landed branch, as the island's chips open them */
function LandedLinks({ name, links }: { name: string; links: LandedBranchLinks | undefined }) {
  const pullRequest = links?.pullRequest
  const robot = links?.robot
  return (
    <>
      {pullRequest !== undefined && (
        <Tooltip>
          <TooltipTrigger
            render={<Button size="xs" variant="ghost" aria-label={`Open Pull Request #${pullRequest.number} of ${name}`} onClick={() => openPullRequestUrl(pullRequest.url)} />}
          >
            <GitPullRequestIcon data-icon="inline-start" />
            {`#${pullRequest.number}`}
          </TooltipTrigger>
          <TooltipContent>Open Pull Request</TooltipContent>
        </Tooltip>
      )}
      {robot !== undefined && (
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                size="icon-xs"
                variant="ghost"
                aria-label={`Open ${robot.mode} of ${name} in Patronus`}
                onClick={() => void openUrl(robot.url).catch((error: unknown) => notifyError("Cannot open the Patronus robot", error))}
              />
            }
          >
            <ShieldCheckIcon />
          </TooltipTrigger>
          <TooltipContent>Open in Patronus</TooltipContent>
        </Tooltip>
      )}
    </>
  )
}
