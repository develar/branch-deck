import type { ReactNode } from "react"
import { ChevronRightIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Spinner } from "@/components/ui/spinner"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { ReviewPlanState } from "@/hooks/useReviewPlan"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { BreakageEntry } from "@/presenters/review-plan"
import {
  formatPairState,
  formatPlanSummary,
  formatRewrittenChanges,
  formatSinceReview,
  presentBreakage,
  presentDependencyChanges,
  presentNewApi,
  presentRepeatedEdits,
} from "@/presenters/review-plan"

/**
 * "Review Summary" above the diffs of a scope of the branch review, with the review plan on (spec/branch/review-plan.spec.md,
 * Status: Experimental). What can break leads: what changed since the user's review, the removed or changed API with
 * uses left outside the change or with uses unknown, then the dependency changes and the new links between modules,
 * the repeated edits and the new API, collapsed, and why the usages are missing. Each entry shows its place in the
 * review: the line of the declaration or of the use, else the file. Rust decides and ranks; the card words what it sent, in its order.
 */
export function ReviewPlanCard({ state }: { state: ReviewPlanState }) {
  const { plan, loading, error } = state
  const since = plan?.interdiff == null ? null : formatSinceReview(plan.interdiff)
  const rewritten = plan?.interdiff?.commits.filter(commit => commit.state === "rewritten" && commit.changes !== null) ?? []
  const breakage = plan === null ? null : presentBreakage(plan.api)
  const dependencies = plan === null ? null : presentDependencyChanges(plan)
  const repeated = plan === null ? [] : presentRepeatedEdits(plan.repeatedEdits)
  const newApi = plan === null ? [] : presentNewApi(plan.api)
  return (
    <section aria-label="Review Summary" data-testid="review-plan-card" className="flex max-h-72 shrink-0 flex-col gap-2 overflow-y-auto border-b border-border px-4 py-3 text-sm">
      <SectionLabel title="Review Summary">
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
      {breakage !== null && (breakage.entries.length > 0 || breakage.updated !== null) && (
        <div className="flex flex-col gap-1" data-testid="review-plan-breakage">
          {breakage.entries.length > 0 && (
            <>
              <SectionLabel title="Possible Breakage" count={breakage.entries.length} />
              <ul aria-label="Possible breakage" className="flex flex-col gap-1.5">
                {breakage.entries.map(entry => (
                  <BreakageItem key={entry.key} entry={entry} />
                ))}
              </ul>
            </>
          )}
          {breakage.updated !== null && (
            <Folded title={breakage.updated.label} testId="review-plan-updated">
              <ul aria-label="API changes with every use updated" className="flex flex-col gap-0.5">
                {breakage.updated.entries.map(entry => (
                  <li key={entry.key} className="flex min-w-0 items-center gap-2" data-testid="review-plan-updated-entry" data-name={entry.name}>
                    <Badge variant={entry.tone}>{entry.kind}</Badge>
                    <FileButton place={entry.place} title={entry.location}>
                      <code className="font-mono">{entry.name}</code>
                      <span className="text-xs text-muted-foreground">{entry.location}</span>
                    </FileButton>
                  </li>
                ))}
              </ul>
            </Folded>
          )}
        </div>
      )}
      {dependencies !== null && (dependencies.modules.length > 0 || dependencies.links.length > 0) && (
        <div className="flex flex-col gap-1" data-testid="review-plan-dependencies">
          <SectionLabel title="Dependencies" />
          {dependencies.modules.map(module => (
            <div key={module.manifest} className="flex min-w-0 flex-col" data-testid="review-plan-dependency-module" data-manifest={module.manifest}>
              <FileButton place={fileTop(module.manifest)} title={module.manifest}>
                <span className="font-medium">{module.module}</span>
                <span className="text-xs text-muted-foreground">{module.manifest}</span>
              </FileButton>
              <ul className="flex flex-col pl-3 text-xs">
                {module.lines.map(line => (
                  <li key={line.key} className="truncate" title={line.text} data-testid="review-plan-dependency" data-change={line.change}>
                    {line.text}
                  </li>
                ))}
              </ul>
            </div>
          ))}
          {dependencies.links.length > 0 && (
            <div className="flex min-w-0 flex-col">
              <span className="text-xs text-muted-foreground">New links between modules</span>
              <ul className="flex flex-col pl-3">
                {dependencies.links.map(link => (
                  <li key={link.key} className="flex min-w-0" data-testid="review-plan-module-link" data-from={link.from} data-to={link.to}>
                    <FileButton place={link.place} title={`${link.from} → ${link.to}`}>
                      <span>{link.label}</span>
                      <span className="text-xs text-muted-foreground">{link.symbols}</span>
                    </FileButton>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
      {repeated.length > 0 && (
        <Folded title="Repeated Edits" count={repeated.length} testId="review-plan-repeated">
          <ul aria-label="Repeated edits" className="flex flex-col gap-1">
            {repeated.map(group => (
              <li key={group.key} className="flex min-w-0 flex-col" data-testid="review-plan-repeated-edit">
                <span className="flex min-w-0 items-baseline gap-1.5">
                  <span className="shrink-0">{`${group.label}:`}</span>
                  <code className="truncate font-mono text-xs" title={group.edit}>
                    {group.edit}
                  </code>
                </span>
                <span className="flex flex-wrap gap-x-2 pl-3 text-xs">
                  {group.files.map(path => (
                    <FileButton key={path} place={fileTop(path)} title={path} testId="review-plan-repeated-file">
                      {path.slice(path.lastIndexOf("/") + 1)}
                    </FileButton>
                  ))}
                </span>
              </li>
            ))}
          </ul>
        </Folded>
      )}
      {newApi.length > 0 && (
        <Folded title="New API" count={newApi.length} testId="review-plan-new-api">
          <ul aria-label="New API" className="flex flex-col gap-0.5">
            {newApi.map(file => (
              <li key={file.path} className="flex min-w-0" data-testid="review-plan-new-api-file" data-path={file.path}>
                <FileButton place={file.place} title={file.path}>
                  <span>{file.path}</span>
                  <code className="truncate font-mono text-xs text-muted-foreground">{file.names}</code>
                </FileButton>
              </li>
            ))}
          </ul>
        </Folded>
      )}
      {plan?.indexNote != null && (
        <p className="text-xs text-muted-foreground" data-testid="review-plan-index-note">
          {`No usages: ${plan.indexNote}`}
        </p>
      )}
    </section>
  )
}

/** A removed or changed API: its kind and name select its file; its forms before and after, then the uses left outside */
function BreakageItem({ entry }: { entry: BreakageEntry }) {
  const sites = entry.moreSites === null ? entry.sites : [...entry.sites, entry.moreSites]
  return (
    <li className="flex min-w-0 flex-col" data-testid="review-plan-breakage-entry" data-name={entry.name} data-path={entry.path}>
      <div className="flex min-w-0 items-center gap-2">
        <Badge variant={entry.tone}>{entry.kind}</Badge>
        <FileButton place={entry.place} title={entry.location} testId="review-plan-breakage-file">
          <code className="font-mono">{entry.name}</code>
          <span className="text-xs text-muted-foreground">{entry.location}</span>
        </FileButton>
        {entry.changed !== null && <span className="shrink-0 text-xs text-muted-foreground">{entry.changed}</span>}
        <span className="ml-auto shrink-0 text-xs text-warning tabular-nums" data-testid="review-plan-outside">
          {entry.outside}
        </span>
      </div>
      {entry.before !== null && (
        <code className="truncate pl-3 font-mono text-xs text-muted-foreground" title={entry.before} data-testid="review-plan-before">
          {`− ${entry.before}`}
        </code>
      )}
      {entry.after !== null && (
        <code className="truncate pl-3 font-mono text-xs text-muted-foreground" title={entry.after} data-testid="review-plan-after">
          {`+ ${entry.after}`}
        </code>
      )}
      {(sites.length > 0 || entry.inside !== null) && (
        <span className="truncate pl-3 text-xs text-muted-foreground" title={sites.join("\n")} data-testid="review-plan-sites">
          {[sites.join(" · "), entry.inside].filter(part => part !== null && part !== "").join(" · ")}
        </span>
      )}
    </li>
  )
}

/** A part of the card that starts collapsed under a muted toggle with its title and count */
function Folded({ title, count, testId, children }: { title: string; count?: number; testId: string; children: ReactNode }) {
  return (
    <Collapsible className="flex flex-col gap-1" data-testid={testId}>
      <CollapsibleTrigger className="group flex min-w-0 items-center gap-1 rounded-sm text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30">
        <ChevronRightIcon className="size-3.5 shrink-0 transition-transform group-data-[panel-open]:rotate-90" />
        <span className="truncate">{title}</span>
        {count !== undefined && <span className="tabular-nums">{count}</span>}
      </CollapsibleTrigger>
      <CollapsibleContent className="pl-4.5">{children}</CollapsibleContent>
    </Collapsible>
  )
}

/**
 * What a click shows in the review: a changed file, by its path or its old path, at the line of the entry; plain text
 * outside a review
 */
function FileButton({ place, title, testId = "review-plan-file", children }: { place: ReviewPlace; title: string; testId?: string; children: ReactNode }) {
  const select = useSelectReviewFile()
  return (
    <button
      type="button"
      className="flex min-w-0 items-baseline gap-2 truncate rounded-sm text-left outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline"
      title={title}
      disabled={select === null}
      data-testid={testId}
      data-path={place.path}
      onClick={() => select?.(place)}
    >
      {children}
    </button>
  )
}
