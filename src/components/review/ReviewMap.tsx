import type { ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { BreakableCode } from "@/components/shared/CodeText"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewPlace } from "@/lib/review-place"
import type { MapRow, MapView } from "@/presenters/review-map"

/**
 * The map of a review (spec/branch/review-tour.spec.md, "The Map"): the shape of the change before its words, a row per
 * module (its badge when the change adds or deletes it, its label, a bar of its share of the changed lines, its counts),
 * under it what it uses or its folders and what it adds beside its code; then the contract in one line and the API it
 * opens wider. Each row ends at the edge of the reading column and truncates, never wraps; a click on a row shows its
 * first file, on a spec its top, on a name its declaration
 */
export function ReviewMap({ view }: { view: MapView }) {
  const { rows, more, contract, api, total } = view
  if (rows.length === 0 && contract === null && api === null) {
    return null
  }
  return (
    <div className="flex min-w-0 flex-col gap-2" data-testid="review-map">
      {rows.length > 0 && (
        <div className="flex min-w-0 flex-col gap-1.5">
          <div className="flex min-w-0 items-baseline gap-2 text-sm">
            <h3 className="font-semibold">Shape</h3>
            <span className="ml-auto text-xs text-muted-foreground" data-testid="review-map-total">
              {total.files === 1 ? "1 file" : `${total.files} files`} <LineCounts additions={total.additions} deletions={total.deletions} colored={false} />
            </span>
          </div>
          <ul aria-label="Modules" className="flex min-w-0 flex-col gap-1.5">
            {rows.map(row => (
              <ShapeRow key={row.key} row={row} />
            ))}
          </ul>
          {more !== null && (
            <Tooltip>
              <TooltipTrigger render={<span className="self-start text-xs text-muted-foreground underline decoration-dotted underline-offset-2" data-testid="review-map-more" />}>
                {more.count === 1 ? "and 1 more module" : `and ${more.count} more modules`}
              </TooltipTrigger>
              <TooltipContent className="max-w-sm">{more.labels}</TooltipContent>
            </Tooltip>
          )}
        </div>
      )}
      {contract !== null && (
        <MapLine label="Contract" testId="review-map-contract">
          {contract.newSpecs.map((spec, index) => (
            <span key={spec.key} className="flex min-w-0 items-baseline gap-1">
              {index > 0 && <span aria-hidden="true">·</span>}
              <span className="shrink-0 text-muted-foreground">New spec</span>
              <PlaceButton place={spec.place} title={spec.key}>
                {spec.name}
              </PlaceButton>
              {spec.rules > 0 && <span className="shrink-0 text-muted-foreground tabular-nums">{spec.rules === 1 ? "(1 requirement)" : `(${spec.rules} requirements)`}</span>}
            </span>
          ))}
          {contract.changed !== null && (
            <span className="flex min-w-0 items-baseline gap-1 text-muted-foreground">
              {contract.newSpecs.length > 0 && <span aria-hidden="true">·</span>}
              <span className="truncate">{contract.changed}</span>
            </span>
          )}
        </MapLine>
      )}
      {api !== null && (
        <MapLine label="API" testId="review-map-api">
          {api.widened.length > 0 && (
            <span className="flex min-w-0 items-baseline gap-1">
              <span className="shrink-0 text-muted-foreground">Opens wider:</span>
              {api.widened.map((name, index) => (
                <span key={name.key} className="flex items-baseline">
                  <PlaceButton place={name.place} title={name.place.path} code>
                    {name.name}
                  </PlaceButton>
                  {index < api.widened.length - 1 && <span className="text-muted-foreground">,</span>}
                </span>
              ))}
            </span>
          )}
          {api.verdict !== null && (
            <span className="flex min-w-0 items-baseline gap-1 text-muted-foreground" data-testid="review-map-verdict">
              {api.widened.length > 0 && <span aria-hidden="true">·</span>}
              {api.verdict}
            </span>
          )}
        </MapLine>
      )}
    </div>
  )
}

/** A row of the shape: its main line and, under it, what it uses or its folders and what it adds beside its code */
function ShapeRow({ row }: { row: MapRow }) {
  const navigator = useSelectReviewFile()
  return (
    <li className="flex min-w-0 flex-col" data-testid="review-map-module" data-root={row.root}>
      <button
        type="button"
        className="grid min-w-0 grid-cols-[minmax(0,1fr)_5rem_auto] items-center gap-3 rounded-sm text-left text-sm outline-none hover:bg-muted/50 focus-visible:bd-focus disabled:hover:bg-transparent"
        title={row.root}
        disabled={navigator === null || row.place === null}
        onClick={() => {
          if (row.place !== null) {
            navigator?.show(row.place)
          }
        }}
      >
        <span className="flex min-w-0 items-center gap-2">
          {row.badge !== null && (
            <Badge variant={row.badge === "New module" ? "success" : "warning"} className="shrink-0" data-testid="review-map-badge">
              {row.badge}
            </Badge>
          )}
          <span className="min-w-0 truncate font-mono font-medium" data-testid="review-map-label">
            {row.label}
          </span>
        </span>
        <span aria-hidden="true" className="h-1 w-full overflow-hidden rounded-full bg-muted">
          <span
            className={cn("block h-full rounded-full", row.badge === "New module" ? "bg-success/60" : "bg-muted-foreground/40")}
            style={{ width: `${Math.max(4, Math.round(row.share * 100))}%` }}
          />
        </span>
        <span className="flex shrink-0 items-baseline gap-2 text-xs text-muted-foreground tabular-nums">
          {(row.additions > 0 || row.deletions > 0) && <LineCounts additions={row.additions} deletions={row.deletions} colored={false} />}
          <span>{row.files === 1 ? "1 file" : `${row.files} files`}</span>
        </span>
      </button>
      {(row.detail !== null || row.extra !== null) && (
        <span className="flex min-w-0 gap-1.5 text-xs text-muted-foreground" data-testid="review-map-detail">
          {row.detail !== null && <span className="min-w-0 truncate">{row.detail}</span>}
          {row.detail !== null && row.extra !== null && <span aria-hidden="true">·</span>}
          {row.extra !== null && <span className="min-w-0 truncate">{row.extra}</span>}
        </span>
      )}
    </li>
  )
}

/** A line of the map under the shape: its label in the gutter of the labels, its parts after it, truncated at the edge */
function MapLine({ label, testId, children }: { label: string; testId: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 items-baseline gap-2 text-sm" data-testid={testId}>
      <h3 className="w-20 shrink-0 font-semibold">{label}</h3>
      <span className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5">{children}</span>
    </div>
  )
}

/** A name of the map that shows its place in the review: a file at its top, a declaration at its line */
function PlaceButton({ place, title, code = false, children }: { place: ReviewPlace; title: string; code?: boolean; children: string }) {
  const navigator = useSelectReviewFile()
  return (
    <button
      type="button"
      className={cn("min-w-0 truncate rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline", code && "font-mono")}
      title={title}
      disabled={navigator === null}
      onClick={() => navigator?.show(place)}
    >
      {code ? <BreakableCode text={children} humps /> : children}
    </button>
  )
}
