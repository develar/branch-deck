import { useEffect } from "react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { LinkedText } from "@/components/commit/LinkedText"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { useNow } from "@/hooks/useNow"
import type { YouTrackIssue } from "@/lib/bindings"
import { getIssueActions } from "@/lib/issue-actions"
import { openSettings } from "@/lib/settings-window"
import { formatRelativeTime } from "@/lib/time"
import { useOperations } from "@/stores/operations"
import { loadIssue, useIssueEntry } from "@/stores/youtrack"

/**
 * The issue of the popover (spec/repository/youtrack.spec.md): its key and summary, its state, its single-value fields
 * by the names YouTrack gives them, and its actions. Asked when it shows.
 */
export function IssueDetails({ issueKey }: { issueKey: string }) {
  useEffect(() => {
    void loadIssue(issueKey)
  }, [issueKey])
  const entry = useIssueEntry(issueKey)
  if (entry === undefined) {
    // a reopened popover shows the issue it knows while it asks again
    return (
      <p className="flex items-center gap-2 text-muted-foreground" data-testid="issue-loading">
        <Spinner className="size-3" />
        {issueKey}
      </p>
    )
  }
  switch (entry.status) {
    case "found":
      return <FoundIssue issue={entry.issue} />
    case "notFound":
      return <Message issueKey={issueKey} text="Not found, or no access" />
    case "error":
      return <Message issueKey={issueKey} text={`Cannot load ${issueKey}: ${entry.message}`} retry />
    case "notSignedIn":
    case "signInProblem":
      return (
        <div className="flex flex-col gap-2" data-testid="issue-sign-in">
          <p className="text-warning">
            {entry.status === "signInProblem" && entry.problem === "expired" ? "Your YouTrack sign-in expired: sign in again" : "Sign in to YouTrack to see the issue"}
          </p>
          <div>
            <Button size="xs" variant="outline" onClick={() => openSettings({ page: "youtrack" })}>
              YouTrack Settings…
            </Button>
          </div>
        </div>
      )
  }
}

function FoundIssue({ issue }: { issue: YouTrackIssue }) {
  const running = useOperations()
  const now = useNow(60_000)
  const state = issue.state
  return (
    <div className="flex flex-col gap-3" data-testid="issue-details" data-resolved={issue.resolved !== null}>
      <div className="flex flex-col gap-1">
        <span className="text-xs text-muted-foreground tabular-nums">{issue.key}</span>
        <LinkedText text={issue.summary} className="font-medium wrap-break-word" />
      </div>
      {state !== null && (
        <p data-testid="issue-state" className={issue.resolved === null ? undefined : "text-success"}>
          {issue.resolved === null ? (state.value ?? "No state") : `${state.value ?? "Resolved"} · resolved ${formatRelativeTime(issue.resolved * 1000, now)}`}
        </p>
      )}
      {issue.fields.length > 0 && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs" data-testid="issue-fields">
          {issue.fields.map(field => (
            <div key={field.name} className="contents">
              <dt className="text-muted-foreground">{field.name}</dt>
              <dd className="min-w-0 truncate">{field.value}</dd>
            </div>
          ))}
        </dl>
      )}
      <div className="flex flex-wrap gap-2">
        {getIssueActions(issue, running).map(item => (
          <MenuItemButton
            key={item.id}
            item={item}
            variant={item.id === "mark-fixed" ? "outline" : "ghost"}
            surface={`issue-popover:${issue.key}:${item.id}`}
            data-testid={`issue-action-${item.id}`}
          />
        ))}
      </div>
    </div>
  )
}

function Message({ issueKey, text, retry = false }: { issueKey: string; text: string; retry?: boolean }) {
  return (
    <div className="flex flex-col gap-2" data-testid="issue-message">
      <span className="text-xs text-muted-foreground tabular-nums">{issueKey}</span>
      <p>{text}</p>
      {retry && (
        <div>
          <Button size="xs" variant="ghost" onClick={() => void loadIssue(issueKey)}>
            Try Again
          </Button>
        </div>
      )}
    </div>
  )
}
