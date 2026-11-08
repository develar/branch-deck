import { Fragment } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { UnbrokenApiList } from "@/components/review/UnbrokenApiList"
import { BreakableCode } from "@/components/shared/CodeText"
import { DisclosureChevron } from "@/components/shared/Disclosure"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewPlace } from "@/lib/review-place"
import { MAP_TEXTS } from "@/presenters/review-map"
import type { MapApi, MapRegistration, MapRow, MapText, MapTexts, MapView } from "@/presenters/review-map"

/**
 * The map of a review (spec/branch/review-tour.spec.md, "The Map"): the shape of the change before its words, a row per
 * module (its badge when the change adds or deletes it, its label, a bar of its share of the changed lines, its counts),
 * under it what it uses or its folders and what it adds beside its code; then the contract in one line, the texts of
 * the UI the change adds, removes or changes (the first quoted, all of them under the line), and the API:
 * what it opens wider that no row names, the verdict that no use left outside breaks folding the changes it covers.
 * Each row ends at the edge of the reading column and truncates, never wraps; a click on a row shows its first file, on
 * a spec its top, on a name its declaration
 */
export function ReviewMap({ view }: { view: MapView }) {
  const { rows, more, contract, texts, api, total } = view
  if (rows.length === 0 && contract === null && texts === null && api === null) {
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
          <ShapeRows rows={rows} more={more} />
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
      {texts !== null && <TextsLine texts={texts} />}
      {api !== null && <ApiLine api={api} />}
    </div>
  )
}

/**
 * The rows of the shape and "and N more modules", the one view of them: in the map, and in the rail of the overview of a
 * pull request (`ReviewRail`), where a click shows the first file of the row in the changes. A list narrower than 24rem
 * (the rail) drops the bars, so the labels keep their width
 */
export function ShapeRows({ rows, more }: Pick<MapView, "rows" | "more">) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <ul aria-label="Modules" className="@container flex min-w-0 flex-col gap-1.5">
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
  )
}

/**
 * A row of the shape: its main line and, under it, what it uses or its folders and what it adds beside its code. In a
 * list narrower than 24rem (the rail) the badge of a module the change adds or deletes leads that line in its tone and
 * its label takes the tone, so the label and the counts keep the main line
 */
function ShapeRow({ row }: { row: MapRow }) {
  const navigator = useSelectReviewFile()
  const tone = row.badge === null ? null : row.badge === "New module" ? "success" : "warning"
  const below = [row.detail, row.extra].filter(part => part !== null)
  return (
    <li className="flex min-w-0 flex-col" data-testid="review-map-module" data-root={row.root}>
      <button
        type="button"
        className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-sm text-left text-sm outline-none hover:bg-muted/50 focus-visible:bd-focus disabled:hover:bg-transparent @sm:grid-cols-[minmax(0,1fr)_5rem_auto]"
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
            <Badge variant={tone ?? undefined} className="hidden shrink-0 @sm:inline-flex" data-testid="review-map-badge">
              {row.badge}
            </Badge>
          )}
          <span
            className={cn("min-w-0 truncate font-mono font-medium", tone === "success" && "@max-sm:text-success", tone === "warning" && "@max-sm:text-warning")}
            data-testid="review-map-label"
          >
            {row.label}
          </span>
        </span>
        <span aria-hidden="true" className="hidden h-1 w-full overflow-hidden rounded-full bg-muted @sm:block">
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
      {(below.length > 0 || row.badge !== null) && (
        <span
          className={cn("flex min-w-0 gap-1.5 text-xs text-muted-foreground", below.length === 0 && "@sm:hidden")}
          title={below.length === 0 ? undefined : below.join(" · ")}
          data-testid="review-map-detail"
        >
          {row.badge !== null && (
            <span className={cn("shrink-0 @sm:hidden", tone === "success" ? "text-success" : "text-warning")} data-testid="review-map-detail-badge">
              {row.badge}
            </span>
          )}
          {below.map((part, index) => (
            <Fragment key={index}>
              {(index > 0 || row.badge !== null) && (
                <span aria-hidden="true" className={cn(index === 0 && "@sm:hidden")}>
                  ·
                </span>
              )}
              <span className="min-w-0 truncate">{part}</span>
            </Fragment>
          ))}
        </span>
      )}
      {row.registrations.length > 0 && (
        // what the module plugs into the platform: a line each, under its row; the rail has no room for them
        <ul aria-label={`Registrations of ${row.label}`} className="hidden min-w-0 flex-col pl-3 text-xs @sm:flex" data-testid="review-map-registrations">
          {row.registrations.map(registration => (
            <li key={registration.key} className="flex min-w-0">
              <RegistrationLine registration={registration} />
            </li>
          ))}
          {row.moreRegistrations > 0 && (
            <li className="text-muted-foreground">{row.moreRegistrations === 1 ? "and 1 more registration" : `and ${row.moreRegistrations} more registrations`}</li>
          )}
        </ul>
      )}
    </li>
  )
}

/** "+ applicationService TranscriptEmbedPlayers → TranscriptEmbedPlayersImpl": a click shows the class it registers */
function RegistrationLine({ registration }: { registration: MapRegistration }) {
  const navigator = useSelectReviewFile()
  return (
    <button
      type="button"
      className="flex min-w-0 items-baseline gap-1.5 rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline"
      title={registration.title}
      disabled={navigator === null}
      data-testid="review-map-registration"
      onClick={() => navigator?.show(registration.place)}
    >
      <ChangeSign sign={registration.sign} />
      <span className="shrink-0 text-muted-foreground">{registration.kind}</span>
      <span className="min-w-0 truncate font-mono">{registration.name}</span>
    </button>
  )
}

/**
 * The line of the texts of the UI: their counts, the first `MAP_TEXTS` quoted, each showing its entry in its bundle,
 * then "and N more", which opens all of them under the line, a line each: the sign of what the change does to it, its
 * words (of a change the old ones struck through before them), its key muted
 */
function TextsLine({ texts }: { texts: MapTexts }) {
  const quoted = texts.texts.slice(0, MAP_TEXTS)
  const rest = texts.texts.length - quoted.length
  const line = (
    <MapLine label="UI texts" testId="review-map-texts">
      <span className="shrink-0 text-muted-foreground">{texts.counts}:</span>
      {quoted.map((text, index) => (
        <span key={text.key} className="flex min-w-0 items-baseline">
          <TextQuote text={text} />
          {index < quoted.length - 1 && <span className="text-muted-foreground">,</span>}
        </span>
      ))}
      {rest > 0 && (
        <CollapsibleTrigger
          className="group flex shrink-0 items-baseline gap-1 rounded-sm text-left text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:bd-focus"
          data-testid="review-map-texts-more"
        >
          <DisclosureChevron />
          {`and ${rest} more`}
        </CollapsibleTrigger>
      )}
    </MapLine>
  )
  if (rest === 0) {
    return line
  }
  return (
    <Collapsible className="flex min-w-0 flex-col gap-1">
      {line}
      <CollapsibleContent hiddenUntilFound className="pl-22">
        <ul aria-label="Texts of the UI" className="flex min-w-0 flex-col gap-0.5 text-xs">
          {texts.texts.map(text => (
            <li key={text.key} className="flex min-w-0 items-baseline gap-1.5" data-testid="review-map-text">
              <ChangeSign sign={text.change === "added" ? "+" : text.change === "removed" ? "−" : "~"} />
              {text.before !== null && (
                <>
                  <span className="min-w-0 shrink truncate text-muted-foreground line-through">{`“${text.before}”`}</span>
                  <span aria-hidden="true" className="shrink-0 text-muted-foreground">
                    →
                  </span>
                </>
              )}
              <TextQuote text={text} />
              <span className="min-w-0 shrink-[2] truncate font-mono text-muted-foreground">{text.name}</span>
            </li>
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

/** A text of the UI quoted, a removed one struck through, a click showing its entry in its bundle */
function TextQuote({ text }: { text: MapText }) {
  const navigator = useSelectReviewFile()
  return (
    <button
      type="button"
      className={cn(
        "max-w-64 min-w-0 truncate rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline",
        text.change === "removed" && "text-muted-foreground line-through",
      )}
      title={text.title}
      disabled={navigator === null}
      data-testid="review-map-text-quote"
      onClick={() => navigator?.show(text.place)}
    >
      {`“${text.words}”`}
    </button>
  )
}

/** "+", "−" or "~" before a line of what the change adds, removes or changes, in the tone of the change */
function ChangeSign({ sign }: { sign: "+" | "−" | "~" }) {
  return (
    <span aria-hidden="true" className={cn("w-2 shrink-0 font-mono", sign === "+" ? "text-success" : sign === "−" ? "text-destructive" : "text-muted-foreground")}>
      {sign}
    </span>
  )
}

/**
 * The API line of the map: what it opens wider that no row names, each showing its declaration, then the verdict that
 * no use left outside breaks, muted, never in the tone of a warning, folding the API changes it covers under the line
 */
function ApiLine({ api }: { api: MapApi }) {
  const line = (
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
        <span className="flex min-w-0 items-baseline gap-1">
          {api.widened.length > 0 && (
            <span aria-hidden="true" className="text-muted-foreground">
              ·
            </span>
          )}
          <CollapsibleTrigger
            className="group flex min-w-0 items-baseline gap-1 rounded-sm text-left text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:bd-focus"
            data-testid="review-map-verdict"
          >
            <DisclosureChevron />
            {api.verdict.label}
          </CollapsibleTrigger>
        </span>
      )}
    </MapLine>
  )
  if (api.verdict === null) {
    return line
  }
  return (
    <Collapsible className="flex min-w-0 flex-col gap-1">
      {line}
      {/* under the parts of the line, past the gutter of its label */}
      <CollapsibleContent hiddenUntilFound className="pl-22 font-medium">
        <UnbrokenApiList entries={api.verdict.entries} />
      </CollapsibleContent>
    </Collapsible>
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
