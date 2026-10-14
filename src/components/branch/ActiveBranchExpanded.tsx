import { cn } from "cn"
import { InfoIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { hasSelectionActions, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { useListFixups } from "@/hooks/useSquashFixups"
import { blockingBranchOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"

export interface ActiveBranchExpandedProps {
  branch: Branch
  /** The amend form is open: mark the tip commit as the amend target */
  highlightTipCommit?: boolean
  /** The nesting depth of the branch row: the rule follows its folder icon */
  depth?: number
}

/** The left margin per nesting level of a branch row (`NestingRules` of `BranchNameCell`) */
const NESTING_STEP_REM = 1.75

export function ActiveBranchExpanded({ branch, highlightTipCommit = false, depth = 0 }: ActiveBranchExpandedProps) {
  const genericError = branch.errorDetails?.Generic
  return (
    // neutral rule under the folder icon; the subjects start at the branch name x
    <div className={cn(depth === 0 && "ml-8", "border-l border-border pr-6")} style={depth === 0 ? undefined : { marginLeft: `${2 + depth * NESTING_STEP_REM}rem` }}>
      {branch.status === "Blocked" && <BlockedAlert branch={branch} />}
      {genericError !== undefined
        ? (
            <Alert variant="destructive">
              <AlertDescription>{genericError}</AlertDescription>
            </Alert>
          )
        : branch.commits.length > 0
          ? <BranchCommitList branch={branch} highlightTipCommit={highlightTipCommit} />
          : <div className="text-sm text-muted-foreground">No commits found</div>}
    </div>
  )
}

/** Why a stacked branch is not rebuilt, with a link to the branch that blocks it */
function BlockedAlert({ branch }: { branch: Branch }) {
  const blocker = blockingBranchOf(branch)
  return (
    <Alert variant="info" data-testid="branch-blocked" className="mb-2">
      <InfoIcon />
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        <span>
          {branch.statusText}
          . The branch keeps its last build until the parent can be built on.
        </span>
        {blocker !== null && (
          <Button variant="link" size="xs" className="h-auto px-0 text-info" onClick={() => requestReveal({ branchName: blocker, commitIds: [] })}>
            {`Show ${blocker}`}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  )
}

/** The commits of the branch; select them to move them to another branch (or drag them onto one). */
function BranchCommitList({ branch, highlightTipCommit }: { branch: Branch, highlightTipCommit: boolean }) {
  const moveList = useCommitMoveList({ commits: branch.commits, sourceBranch: branch.name })
  const { count: fixupCount } = useListFixups(branch.name)
  return (
    <div className="flex flex-col gap-2">
      {(hasSelectionActions(moveList) || fixupCount > 0) && (
        <div className="flex items-center justify-end gap-2 px-4 sm:px-6">
          <CommitSelectionActions moveList={moveList} />
          <SquashFixupsButton branch={branch.name} />
        </div>
      )}
      <MovableCommitList moveList={moveList} commits={branch.commits} variant="status" showFileCount={false} highlightTipCommit={highlightTipCommit} editable />
    </div>
  )
}
