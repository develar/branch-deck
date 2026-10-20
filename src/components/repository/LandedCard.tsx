import { useEffect, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon, PackageCheckIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { CommitSubject, CommitSubjectLine } from "@/components/commit/CommitSubject"
import { CopyButton } from "@/components/shared/CopyButton"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { LandedBranch, LandedCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { landedBranchName, landedCommitter, landedCountLabel, sharedCommitter } from "@/lib/landed"
import { useBaselineStatus } from "@/stores/branch-sync"
import { dismissLanded, markLandedSeen, useLandedIsland } from "@/stores/landed"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * The island of what landed in the baseline with an update (here or elsewhere), above the branches: a toast passes, the
 * news stays until the user has seen it, through the next sync at least 10 s later, or dismissed it; Activity keeps it.
 * A row per branch with the count, the robot or pull request that merged it and the committer (once in the header when
 * every branch has the same one); it expands to the commits, each with the baseline commit that carries it.
 */
export function LandedCard() {
  const report = useLandedIsland()
  const repositoryPath = useSelectedProjectPath()
  const status = useBaselineStatus()
  const landedAt = report.length === 0 ? 0 : Math.max(...report.map(branch => branch.landedAt))
  // the user sees the island while the window has focus: a sync after that hides it
  useEffect(() => {
    if (repositoryPath !== undefined && landedAt !== 0) {
      markLandedSeen(repositoryPath, landedAt)
    }
  }, [repositoryPath, landedAt])
  useWindowFocus(() => {
    if (repositoryPath !== undefined && landedAt !== 0) {
      markLandedSeen(repositoryPath, landedAt)
    }
  })
  if (report.length === 0 || repositoryPath === undefined) {
    return null
  }

  const upstream = status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
  const committer = sharedCommitter(report)
  const dismiss = () => void dismissLanded(repositoryPath)
  return (
    <Card className="animate-in gap-0 divide-y divide-border py-0 duration-300 fade-in-0 slide-in-from-top-2" data-testid="landed-card">
      <div className="flex items-center justify-between gap-3 px-4 py-2.5 sm:px-6">
        <div className="flex min-w-0 items-center gap-2" data-testid="landed-header">
          <PackageCheckIcon className="size-4 shrink-0 text-info" />
          <h2 className="truncate text-sm font-semibold text-foreground">{`Landed in ${upstream}`}</h2>
          <span className="shrink-0 text-xs text-muted-foreground">
            {`· ${committer === undefined ? "" : `by ${committer} · `}`}
            <RelativeTime time={landedAt * 1000} />
          </span>
        </div>
        <Tooltip>
          <TooltipTrigger render={<Button size="icon-xs" variant="ghost" aria-label="Dismiss" data-testid="landed-dismiss" onClick={dismiss} />}>
            <XIcon />
          </TooltipTrigger>
          <TooltipContent>Dismiss</TooltipContent>
        </Tooltip>
      </div>
      <ul className="divide-y divide-border">
        {report.map(branch => (
          <LandedBranchRow key={branch.name ?? ""} branch={branch} showCommitter={committer === undefined} />
        ))}
      </ul>
    </Card>
  )
}

export interface LandedBranchRowProps {
  branch: LandedBranch
  showCommitter: boolean
  /** In a list (Activity): the paddings of a list there, not of the island */
  compact?: boolean
  /** What the row shows of the branch's pull request and robot: the chips of the main window's stores by default */
  renderLinks?: (name: string) => ReactNode
}

/** A branch of a landed report: the count, the review chips and the committer; it expands to the commits */
export function LandedBranchRow({ branch, showCommitter, compact = false, renderLinks = renderChips }: LandedBranchRowProps) {
  const [expanded, setExpanded] = useState(false)
  const committer = showCommitter ? landedCommitter(branch) : undefined
  return (
    <li data-testid="landed-branch" data-branch-name={branch.name ?? undefined}>
      <div className={cn("flex items-center gap-3", compact ? "px-3 py-2" : "px-4 py-2.5 sm:px-6")}>
        <button
          type="button"
          className="flex min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-lg text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          <ChevronRightIcon className={cn("size-4 shrink-0 text-muted-foreground transition-transform duration-200", expanded && "rotate-90")} />
          <span className={cn("truncate font-medium", branch.name === null && "text-muted-foreground")}>{landedBranchName(branch)}</span>
          <span className="shrink-0 text-muted-foreground" data-testid="landed-count">
            {landedCountLabel(branch)}
          </span>
        </button>
        {branch.name !== null && <div className="flex shrink-0 items-center gap-2">{renderLinks(branch.name)}</div>}
        {committer !== undefined && <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">{`by ${committer}`}</span>}
      </div>
      {expanded && (
        <ul className={cn("flex flex-col gap-1 bg-muted/50 py-2", compact ? "pr-3 pl-9" : "pr-4 pl-12 sm:pr-6")} data-testid="landed-commits">
          {branch.commits.map(commit => (
            <LandedCommitRow key={commit.id} commit={commit} compact={compact} />
          ))}
        </ul>
      )}
    </li>
  )
}

/** The chips of the branch's pull request and robot, from the stores of the main window */
function renderChips(name: string): ReactNode {
  return (
    <>
      <PullRequestChip branchName={name} />
      <PatronusChip branchName={name} />
    </>
  )
}

function LandedCommitRow({ commit, compact }: { commit: LandedCommit; compact: boolean }) {
  const upstream = commit.upstream
  return (
    <li className="group flex items-center gap-3 text-xs">
      {/* a popover list has one line per commit */}
      <span className={cn("min-w-0 flex-1", compact && "flex")}>{compact ? <CommitSubjectLine subject={commit.subject} /> : <CommitSubject subject={commit.subject} />}</span>
      {upstream === null ? (
        <span className="shrink-0 text-muted-foreground">squashed upstream</span>
      ) : (
        <span className="flex shrink-0 items-center gap-2 text-muted-foreground">
          <span className="font-mono" data-testid="landed-upstream-hash">
            {formatShortHash(upstream.id)}
          </span>
          <CopyButton text={() => upstream.id} tooltip="Copy the baseline commit" />
          <span className="hidden sm:inline">
            {`${upstream.committer} · `}
            <RelativeTime time={upstream.time * 1000} />
          </span>
        </span>
      )}
    </li>
  )
}
