import { cn } from "cn"
import { PushUpToMark } from "@/components/commit/PushUpToMark"
import { getCommitSubject, stripLeadingIssueKey } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { isLandedCommit } from "@/lib/patronus"
import { formatCompactTimestamp } from "@/lib/time"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { useLandedBy } from "@/stores/patronus"
import { useIsCommitPushedUpTo } from "@/stores/stack-view"

/**
 * The commits of a branch of a stack island as one line each, oldest first, on the lane of the branch (`bd-lane-rail`
 * draws a dot per `tr[data-row-id]`): time runs down with the graph, so the tip of a parent sits right above the row of
 * its child. A commit the remote lacks (`unpushedCommits`) has the push arrow, which offers Push Up to Here
 * (`PushUpToMark`); one on the remote is muted, and so is one a merged Safe Push landed (`isLandedCommit`, all but
 * the commits added after the landed ones): it is in the target, a push of it lands nothing, and Push Up to Here offers
 * nothing for it on any surface (`pushUpToPlan`). No files, no selection: a click on the branch row opens the full list.
 */
export function StackCommitLines({ branch }: { branch: Branch }) {
  const unpushed = new Set(branch.remoteStatus?.unpushedCommits ?? [])
  // before the remote status loads nothing is marked
  const known = branch.remoteStatus != null
  const landedBy = useLandedBy(branch.name)
  return (
    <table className="w-full table-fixed" data-testid="stack-commit-lines">
      <tbody>
        {branch.commits.toReversed().map(commit => {
          const landed = isLandedCommit(landedBy, commit.originalHash)
          return (
            <CommitLine
              key={commit.originalHash}
              branch={branch}
              commit={commit}
              known={known}
              landed={landed}
              unpushed={known && !landed && commit.hash !== undefined && unpushed.has(commit.hash)}
            />
          )
        })}
      </tbody>
    </table>
  )
}

interface CommitLineProps {
  branch: Branch
  commit: SyncedCommit
  /** The remote status is loaded */
  known: boolean
  /** A merged Safe Push landed the commit: no push arrow */
  landed: boolean
  /** The remote lacks the commit, and it didn't land */
  unpushed: boolean
}

/** A commit line, tinted in the push colour while the hovered Push Up to Here sends it */
function CommitLine({ branch, commit, known, landed, unpushed }: CommitLineProps) {
  const sent = useIsCommitPushedUpTo(commit.hash)
  const subject = branch.issueKey === null ? getCommitSubject(commit) : stripLeadingIssueKey(getCommitSubject(commit), branch.issueKey)
  return (
    <tr
      data-row-id={commit.originalHash}
      data-unpushed={unpushed || undefined}
      data-landed={landed || undefined}
      data-sent={sent || undefined}
      data-testid="stack-commit-line"
      className={cn(sent && "bg-primary/5")}
    >
      <td className="py-1.5">
        <div className="flex min-w-0 items-center gap-2 text-sm leading-5">
          <span className="flex w-4 shrink-0 justify-center">{unpushed && <PushUpToMark branchName={branch.name} originalHash={commit.originalHash} />}</span>
          <span className={cn("min-w-0 flex-1 truncate", (landed || (known && !unpushed)) && "text-muted-foreground")}>{subject}</span>
          <span className="shrink-0 font-mono text-xs text-muted-foreground">{formatShortHash(commit.hash ?? commit.originalHash)}</span>
          <span className="w-24 shrink-0 text-right text-xs text-muted-foreground tabular-nums">{formatCompactTimestamp(commit.authorTime)}</span>
        </div>
      </td>
    </tr>
  )
}
