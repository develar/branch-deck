import { ArrowUpIcon } from "lucide-react"
import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { getCommitSubject, stripLeadingIssueKey } from "@/lib/commit"
import { formatRelativeTime } from "@/lib/time"
import type { Branch } from "@/stores/branch-sync"

/**
 * The commits of a branch of a stack island as one line each, oldest first, on the lane of the branch (`bd-lane-rail`
 * draws a dot per `tr[data-row-id]`): time runs down with the graph, so the tip of a parent sits right above the row of
 * its child. A commit the remote lacks (`unpushedCommits`) has the push arrow; one on the remote is muted. No files, no
 * selection: a click on the branch row opens the full list.
 */
export function StackCommitLines({ branch }: { branch: Branch }) {
  const unpushed = new Set(branch.remoteStatus?.unpushedCommits ?? [])
  // before the remote status loads nothing is marked
  const known = branch.remoteStatus != null
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return (
    <table className="w-full table-fixed" data-testid="stack-commit-lines">
      <tbody>
        {branch.commits.toReversed().map((commit) => {
          const isUnpushed = known && commit.hash !== undefined && unpushed.has(commit.hash)
          const subject = branch.issueKey === null ? getCommitSubject(commit) : stripLeadingIssueKey(getCommitSubject(commit), branch.issueKey)
          return (
            <tr key={commit.originalHash} data-row-id={commit.originalHash} data-unpushed={isUnpushed || undefined} data-testid="stack-commit-line">
              <td className="py-1.5">
                <div className="flex min-w-0 items-center gap-2 text-sm leading-5">
                  <span className="flex w-3 shrink-0 justify-center">
                    {isUnpushed && (
                      <Tooltip>
                        <TooltipTrigger render={<span className="text-primary" aria-label="Not on the remote" data-testid="commit-push-mark" />}>
                          <ArrowUpIcon className="size-3" />
                        </TooltipTrigger>
                        <TooltipContent>Not on the remote: the next push sends it</TooltipContent>
                      </Tooltip>
                    )}
                  </span>
                  <span className={cn("min-w-0 flex-1 truncate", known && !isUnpushed && "text-muted-foreground")}>{subject}</span>
                  <span className="shrink-0 font-mono text-xs text-muted-foreground">{(commit.hash ?? commit.originalHash).slice(0, 8)}</span>
                  <span className="w-20 shrink-0 text-right text-xs text-muted-foreground">{formatRelativeTime(commit.authorTime * 1000, now)}</span>
                </div>
              </td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}
