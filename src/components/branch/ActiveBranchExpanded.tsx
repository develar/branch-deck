import type { ReactNode } from "react"
import { cn } from "cn"
import { InfoIcon, Layers2Icon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { NestingRules } from "@/components/branch/BranchNameCell"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"
import { hasSelectionActions, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { useListFixups } from "@/hooks/useSquashFixups"
import { restackBranch } from "@/hooks/useStackActions"
import { blockingBranchOf, describeParentState, passedParentOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"

export interface ActiveBranchExpandedProps {
  branch: Branch
  /** The amend form is open: mark the tip commit as the amend target */
  highlightTipCommit?: boolean
  /** The nesting depth of the branch row: the rail follows its folder icon */
  depth?: number
  /** The next rows stack on the branch: the rail runs to the bottom of the cell, into their rule */
  hasChildren?: boolean
}

export function ActiveBranchExpanded({ branch, highlightTipCommit = false, depth = 0, hasChildren = false }: ActiveBranchExpandedProps) {
  const genericError = branch.errorDetails?.Generic
  return (
    <CommitRail depth={depth} hasChildren={hasChildren}>
      {branch.status === "Blocked" && <BlockedAlert branch={branch} />}
      {branch.status !== "Blocked" && <PassedParentAlert branch={branch} />}
      {genericError !== undefined
        ? (
            <Alert variant="destructive">
              <AlertDescription>{genericError}</AlertDescription>
            </Alert>
          )
        : branch.commits.length > 0
          ? <BranchCommitList branch={branch} highlightTipCommit={highlightTipCommit} />
          : <div className="text-sm text-muted-foreground">No commits found</div>}
    </CommitRail>
  )
}

/**
 * The neutral rail of the commits under the folder icon of the row, so the subjects start at the branch name. A nested
 * branch keeps the rules of its levels beside it (`NestingRules`), and the rail of a parent runs into the rule of its
 * children (`--bd-nesting-step` in index.css).
 */
function CommitRail({ depth, hasChildren, children }: { depth: number, hasChildren: boolean, children: ReactNode }) {
  return (
    <div className="pl-8 animate-in duration-150 fade-in-0">
      <NestingRules levels={depth}>
        <div className={cn("flex flex-col gap-2 border-l border-border pr-6", hasChildren && "-mb-(--bd-cell-y) pb-(--bd-cell-y)")} data-testid="commit-rail">
          {children}
        </div>
      </NestingRules>
    </div>
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
        <span>
          {branch.statusText}
          . The branch keeps its last build until the parent can be built on.
        </span>
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
 * landed. The pull request base is master." The tone follows the parent: `info` landed (in the baseline), `warning` no
 * commits (likely a typo), neutral unapplied. The only fix that rewrites prefixes is the user's: Unstack.
 */
function PassedParentAlert({ branch }: { branch: Branch }) {
  const baseline = useBaselineOnRemote()
  const passed = passedParentOf(branch)
  if (passed === null) {
    return null
  }
  const base = branch.stack?.effectiveBase ?? baseline
  const variant = passed.state === "Integrated" ? "info" : passed.state === "Missing" ? "warning" : "default"
  const linkTone = passed.state === "Integrated" ? "text-info" : passed.state === "Missing" ? "text-warning" : undefined
  return (
    <Alert variant={variant} data-testid="branch-base-changed">
      {passed.state === "Missing" ? <TriangleAlertIcon /> : <InfoIcon />}
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        <span>{`Built on ${base}: ${describeParentState(passed.parent, passed.state)}. The pull request base is ${base}.`}</span>
        <Button variant="link" size="xs" className={cn("h-auto px-0", linkTone)} onClick={() => void restackBranch(branch.name, null)}>
          <Layers2Icon data-icon="inline-start" />
          {`Unstack from ${passed.parent}`}
        </Button>
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
      <MovableCommitList moveList={moveList} commits={branch.commits} branchName={branch.name} variant="status" showFileCount={false} highlightTipCommit={highlightTipCommit} editable />
    </div>
  )
}
