import { useMemo } from "react"
import type { CSSProperties, ReactNode } from "react"
import { cn } from "cn"
import { InfoIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { FamilyGuide, LanesBelow } from "@/components/branch/StackLanes"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"
import { getBranchUnstackItem } from "@/hooks/useBranchContextActions"
import { hasSelectionActions, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { useListFixups } from "@/hooks/useSquashFixups"
import { getCommitHash } from "@/lib/commit"
import { robotRailMarks } from "@/lib/robot-coverage"
import { blockingBranchOf, describeParentState, passedParentOf } from "@/lib/stack"
import type { LaneSegments, StackLanes } from "@/lib/stack-lanes"
import { laneContentInsetClass } from "@/lib/stack-lanes"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useOperations } from "@/stores/operations"
import { useRobotCoverage } from "@/stores/robot-coverage"

export interface ActiveBranchExpandedProps {
  branch: Branch
  /** The amend form is open: mark the tip commit as the amend target */
  highlightTipCommit?: boolean
  /** Under the row of its issue: the commits are one step further in, beside the family guide */
  familyIndent?: 0 | 1
  /** The next rows are the members of its issue family: the guide of the family runs beside the commits into theirs */
  familyBelow?: boolean
  /** The place of the branch in the stack graph: the commits sit on its lane; null in a table without stacks */
  lanes: StackLanes
  /** The segments of a landing span in the row (`landingSegments`): the lanes of the span run on beside the commits */
  landing?: LaneSegments | null
}

export function ActiveBranchExpanded({ branch, highlightTipCommit = false, familyIndent = 0, familyBelow = false, lanes, landing = null }: ActiveBranchExpandedProps) {
  const genericError = branch.errorDetails?.Generic
  return (
    <CommitRail name={branch.name} familyIndent={familyIndent} familyBelow={familyBelow} lanes={lanes} landing={landing}>
      {branch.status === "Blocked" && <BlockedAlert branch={branch} />}
      {branch.status !== "Blocked" && <PassedParentAlert branch={branch} />}
      {genericError !== undefined ? (
        <Alert variant="destructive">
          <AlertDescription>{genericError}</AlertDescription>
        </Alert>
      ) : branch.commits.length > 0 ? (
        <BranchCommitList branch={branch} highlightTipCommit={highlightTipCommit} oldestFirst={lanes.alone !== true} />
      ) : (
        <div className="text-sm text-muted-foreground">No commits found</div>
      )}
    </CommitRail>
  )
}

export interface CommitRailProps {
  /** The branch: the highlighted path to the root of its stack runs on beside its commits */
  name: string
  familyIndent: 0 | 1
  familyBelow: boolean
  lanes: StackLanes
  landing: LaneSegments | null
  children: ReactNode
}

/**
 * The commits of an expanded branch, their subjects at the branch name, on the lane of the branch: a dot per commit
 * (`bd-lane-rail` in index.css), beside the lanes that run on below the row; the lane of the branch runs on into its
 * children, or ends at the last commit; a family member keeps its guide beside them.
 */
export function CommitRail({ name, familyIndent, familyBelow, lanes, landing, children }: CommitRailProps) {
  return (
    <>
      <LanesBelow name={name} lanes={lanes} commitsBelow={false} landing={landing} />
      {(familyIndent === 1 || familyBelow) && <FamilyGuide />}
      <div
        className={cn("bd-lane-rail animate-in py-(--bd-cell-y) duration-150 fade-in-0", laneContentInsetClass(familyIndent))}
        style={{ "--bd-rail-lane": lanes.lane } as CSSProperties}
        data-lane-end={lanes.down ? undefined : ""}
      >
        <div className="flex flex-col gap-2 pr-6" data-testid="commit-rail">
          {children}
        </div>
      </div>
    </>
  )
}

/**
 * Why a stacked branch is not rebuilt, with a link to the branch that blocks it: `warning`, as the warning dot of its
 * status and the Blocked badge of its commits
 */
function BlockedAlert({ branch }: { branch: Branch }) {
  const blocker = blockingBranchOf(branch)
  return (
    <Alert variant="warning" data-testid="branch-blocked">
      <TriangleAlertIcon />
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        <span>{branch.statusText}. The branch keeps its last build until the parent can be built on.</span>
        {blocker !== null && (
          <Button variant="link" size="xs" className="h-auto px-0 text-warning" onClick={() => requestReveal({ branchName: blocker, commitIds: [] })}>
            {`Show ${blocker}`}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  )
}

/**
 * Where a stacked branch is built while its declared parent has no commits, and why: "Built on master: air-misc
 * landed." The tone follows the parent: `info` landed (in the baseline), `warning` no commits (likely a typo), neutral
 * unapplied. The only fix that rewrites prefixes is the user's: Unstack, the item of the branch's menu
 * (`getBranchUnstackItem`), its spinner here while it runs.
 */
function PassedParentAlert({ branch }: { branch: Branch }) {
  const baseline = useBaselineOnRemote()
  const operations = useOperations()
  const passed = passedParentOf(branch)
  const unstack = getBranchUnstackItem(branch, operations)
  if (passed === null || unstack === null) {
    return null
  }
  const base = branch.stack?.effectiveBase ?? baseline
  const variant = passed.state === "Integrated" ? "info" : passed.state === "Missing" ? "warning" : "default"
  const linkTone = passed.state === "Integrated" ? "text-info" : passed.state === "Missing" ? "text-warning" : undefined
  return (
    <Alert variant={variant} data-testid="branch-base-changed">
      {passed.state === "Missing" ? <TriangleAlertIcon /> : <InfoIcon />}
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        <span>{`Built on ${base}: ${describeParentState(passed.parent, passed.state)}.`}</span>
        <MenuItemButton item={unstack} variant="link" surface={`unstack-alert:${branch.name}`} className={cn("h-auto px-0", linkTone)} data-testid="unstack-from-parent" />
      </AlertDescription>
    </Alert>
  )
}

/**
 * The commits of the branch; select them to move them to another branch (or drag them onto one). On the lane of a stack
 * (`oldestFirst`) time runs down with the graph: the tip of a parent sits right above the row of its child.
 */
function BranchCommitList({ branch, highlightTipCommit, oldestFirst }: { branch: Branch; highlightTipCommit: boolean; oldestFirst: boolean }) {
  const commits = useMemo(() => (oldestFirst ? branch.commits.toReversed() : branch.commits), [branch.commits, oldestFirst])
  const moveList = useCommitMoveList({ commits, oldestFirst, sourceBranch: branch.name })
  const { count: fixupCount } = useListFixups(branch.name)
  // the running robot that tests the branch: its line over the commits it covers, a marker on the ones it lacks
  const coverage = useRobotCoverage(branch.name)
  const robotMarks = useMemo(() => (coverage === null ? undefined : robotRailMarks(commits.map(getCommitHash), coverage, !oldestFirst)), [commits, coverage, oldestFirst])
  return (
    <div className="flex flex-col gap-2">
      {(hasSelectionActions(moveList) || fixupCount > 0) && (
        <div className="flex items-center justify-end gap-2 px-4 sm:px-6">
          <CommitSelectionActions moveList={moveList} />
          <SquashFixupsButton branch={branch.name} />
        </div>
      )}
      <MovableCommitList
        moveList={moveList}
        commits={commits}
        branchName={branch.name}
        variant="status"
        showFileCount={false}
        highlightTipCommit={highlightTipCommit}
        oldestFirst={oldestFirst}
        omitIssueKey={branch.issueKey}
        editable
        robotMarks={robotMarks}
      />
    </div>
  )
}
