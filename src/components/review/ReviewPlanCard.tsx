import { ShieldAlertIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { ReviewPlanState } from "@/hooks/useReviewPlan"
import { formatPairState, formatPlanSummary, formatRewrittenChanges, formatSinceReview, presentApiChanges, presentRiskyFiles } from "@/presenters/review-plan"

/**
 * "API & Risk" above the diffs of a scope of the branch review, with the review plan on (spec/branch/review-plan.spec.md,
 * Status: Experimental): what changed since the user's review, the public declarations the reviewed commits add, remove
 * or change with their usages at the head (the most first), and the files changed with used API or without their
 * tests; why the usages are missing when accio cannot tell. Rust ranks; the card words what it sent, in its order.
 */
export function ReviewPlanCard({ state }: { state: ReviewPlanState }) {
  const { plan, loading, error } = state
  const since = plan?.interdiff == null ? null : formatSinceReview(plan.interdiff)
  const rewritten = plan?.interdiff?.commits.filter(commit => commit.state === "rewritten" && commit.changes !== null) ?? []
  const api = plan === null ? [] : presentApiChanges(plan.api)
  const risky = plan === null ? [] : presentRiskyFiles(plan.files)
  return (
    <section aria-label="API & Risk" data-testid="review-plan-card" className="flex max-h-72 shrink-0 flex-col gap-2 overflow-y-auto border-b border-border px-4 py-3 text-sm">
      <SectionLabel title="API & Risk">
        <span className="flex items-center gap-1.5" data-testid="review-plan-summary">
          {loading && <Spinner className="size-3" />}
          {plan === null ? (loading ? "Reading the review plan…" : null) : formatPlanSummary(plan)}
        </span>
      </SectionLabel>
      {error !== null && (
        <p className="text-destructive" data-testid="review-plan-error">
          {`Cannot read the review plan: ${error}`}
        </p>
      )}
      {since !== null && (
        <div className="flex flex-col gap-0.5" data-testid="review-plan-since">
          <span>{`Since your review: ${since}`}</span>
          {rewritten.map(commit => (
            <span key={commit.id} className="truncate text-xs text-muted-foreground" title={commit.subject}>
              {`${formatPairState(commit.state)} ${commit.subject}: ${formatRewrittenChanges(commit.changes!)}`}
            </span>
          ))}
        </div>
      )}
      {api.length > 0 && (
        <ul aria-label="API changes" className="flex flex-col gap-1">
          {api.map(entry => (
            <li key={entry.key} className="flex min-w-0 flex-col" data-testid="review-plan-api-entry" data-name={entry.name}>
              <div className="flex min-w-0 items-center gap-2">
                <Badge variant={entry.tone}>{entry.kind}</Badge>
                <code className="font-mono">{entry.name}</code>
                <span className="min-w-0 truncate text-xs text-muted-foreground" title={entry.location}>
                  {entry.location}
                </span>
                {entry.changed !== null && <span className="shrink-0 text-xs text-muted-foreground">{entry.changed}</span>}
                {entry.fanIn !== null && (
                  <span className="ml-auto shrink-0 text-xs tabular-nums" data-testid="review-plan-fan-in">
                    {entry.fanIn}
                  </span>
                )}
              </div>
              {(entry.before !== null || entry.after !== null) && (
                <code className="truncate font-mono text-xs text-muted-foreground" title={[entry.before, entry.after].filter(it => it !== null).join("\n")}>
                  {entry.before !== null && entry.after !== null ? `${entry.before} → ${entry.after}` : (entry.after ?? entry.before)}
                </code>
              )}
            </li>
          ))}
        </ul>
      )}
      {risky.length > 0 && (
        <ul aria-label="Risky files" className="flex flex-col gap-1">
          {risky.map(file => (
            <li key={file.path} className="flex min-w-0 items-center gap-2" data-testid="review-plan-risk-entry" data-path={file.path}>
              <ShieldAlertIcon className="size-3.5 shrink-0 text-warning" />
              <span className="min-w-0 truncate" title={file.path}>
                {file.path}
              </span>
              <span className="ml-auto shrink-0 text-xs text-muted-foreground">{file.reasons}</span>
            </li>
          ))}
        </ul>
      )}
      {plan?.indexNote != null && (
        <p className="text-xs text-muted-foreground" data-testid="review-plan-index-note">
          {`No usages: ${plan.indexNote}`}
        </p>
      )}
    </section>
  )
}
