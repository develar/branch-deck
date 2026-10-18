import { FileDiffIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Callout } from "@/components/shared/Callout"
import { formatMissingCommitsReason, formatMissingCommitsTitle } from "@/lib/merge-conflict"
import type { MissingCommitOwner, MissingCommitOwners } from "@/lib/merge-conflict"
import { cutList } from "@/lib/text"
import { requestReveal } from "@/stores/commit-move"

/** The owner chips shown before "+N branches" */
const SHOWN_OWNERS = 8

export interface ConflictExplanationProps {
  owners: MissingCommitOwners
  branchName?: string
  /** The branch the commit is copied onto: the stack parent or the baseline */
  builtOn?: string | null
  /** The chips show the branch with its missing commits; a sub-window can't, it names them only */
  revealable: boolean
}

/**
 * Why a commit conflicts: the commits outside its branch that changed the conflicting files first, by the branch they
 * are in, and what the branch is built on. The expanded row and the missing-commits window show the same callout.
 */
export function ConflictExplanation({ owners, branchName, builtOn, revealable }: ConflictExplanationProps) {
  const reason = formatMissingCommitsReason(owners, branchName, builtOn)
  // never "+1 branch": one more chip costs no more than the count (`cutList`)
  const { shown, more: hidden } = cutList(owners.branches, SHOWN_OWNERS)
  return (
    <Callout tone="warning" icon={FileDiffIcon} title={formatMissingCommitsTitle(owners)} data-testid="conflict-explanation">
      {reason !== null && <p>{reason}</p>}
      {owners.branches.length > 0 && (
        <div className="flex flex-wrap items-center gap-1" data-testid="missing-commit-owners">
          {shown.map(owner => (
            <OwnerChip key={owner.branch} owner={owner} revealable={revealable} />
          ))}
          {hidden > 0 && <span className="px-1 text-xs">{`+${hidden} ${hidden === 1 ? "branch" : "branches"}`}</span>}
        </div>
      )}
    </Callout>
  )
}

function OwnerChip({ owner, revealable }: { owner: MissingCommitOwner; revealable: boolean }) {
  const label = (
    <>
      <span className="text-foreground">{owner.branch}</span>
      <span className="text-muted-foreground tabular-nums">{owner.commitIds.length}</span>
    </>
  )
  if (!revealable) {
    return <span className="inline-flex h-6 items-center gap-1.5 rounded-md bg-muted px-2 text-xs">{label}</span>
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={<Button variant="secondary" size="xs" data-owner={owner.branch} onClick={() => requestReveal({ branchName: owner.branch, commitIds: owner.commitIds })} />}
      >
        {label}
      </TooltipTrigger>
      <TooltipContent>{`Show ${owner.branch}`}</TooltipContent>
    </Tooltip>
  )
}
