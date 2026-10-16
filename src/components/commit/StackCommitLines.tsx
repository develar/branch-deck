import { cn } from "cn"
import { PushUpToMark } from "@/components/commit/PushUpToMark"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { getCommitSubject, stripLeadingIssueKey } from "@/lib/commit"
import { formatRelativeTime } from "@/lib/time"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { useIsCommitPushedUpTo } from "@/stores/stack-view"

/**
 * The commits of a branch of a stack island as one line each, oldest first, on the lane of the branch (`bd-lane-rail`
 * draws a dot per `tr[data-row-id]`): time runs down with the graph, so the tip of a parent sits right above the row of
 * its child. A commit the remote lacks (`unpushedCommits`) has the push arrow, which offers Push Up to Here
 * (`PushUpToMark`); one on the remote is muted. No files, no selection: a click on the branch row opens the full list.
 */
export function StackCommitLines({ branch }: { branch: Branch }) {
  const unpushed = new Set(branch.remoteStatus?.unpushedCommits ?? [])
  // before the remote status loads nothing is marked
  const known = branch.remoteStatus != null
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return (
    <table className="w-full table-fixed" data-testid="stack-commit-lines">
      <tbody>
        {branch.commits.toReversed().map(commit => (
          <CommitLine
            key={commit.originalHash}
            branch={branch}
            commit={commit}
            known={known}
            unpushed={known && commit.hash !== undefined && unpushed.has(commit.hash)}
            now={now}
          />
        ))}
      </tbody>
    </table>
  )
}

interface CommitLineProps {
  branch: Branch
  commit: SyncedCommit
  /** The remote status is loaded */
  known: boolean
  /** The remote lacks the commit */
  unpushed: boolean
  now: number
}

/** A commit line, tinted in the push colour while the hovered Push Up to Here sends it */
function CommitLine({ branch, commit, known, unpushed, now }: CommitLineProps) {
  const sent = useIsCommitPushedUpTo(commit.hash)
  const subject = branch.issueKey === null ? getCommitSubject(commit) : stripLeadingIssueKey(getCommitSubject(commit), branch.issueKey)
  return (
    <tr
      data-row-id={commit.originalHash}
      data-unpushed={unpushed || undefined}
      data-sent={sent || undefined}
      data-testid="stack-commit-line"
      className={cn(sent && "bg-primary/5")}
    >
      <td className="py-1.5">
        <div className="flex min-w-0 items-center gap-2 text-sm leading-5">
          <span className="flex w-4 shrink-0 justify-center">{unpushed && <PushUpToMark branchName={branch.name} originalHash={commit.originalHash} />}</span>
          <span className={cn("min-w-0 flex-1 truncate", known && !unpushed && "text-muted-foreground")}>{subject}</span>
          <span className="shrink-0 font-mono text-xs text-muted-foreground">{(commit.hash ?? commit.originalHash).slice(0, 8)}</span>
          <span className="w-20 shrink-0 text-right text-xs text-muted-foreground">{formatRelativeTime(commit.authorTime * 1000, now)}</span>
        </div>
      </td>
    </tr>
  )
}
