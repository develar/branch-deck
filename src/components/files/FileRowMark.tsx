import { cn } from "cn"
import { InfoIcon, TriangleAlertIcon } from "lucide-react"
import { PopoverSectionTitle } from "@/components/ui/popover"
import { RowCard } from "@/components/shared/RowCard"
import type { RowCardState } from "@/components/shared/RowCard"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { LinkEntry, RowFacts, RowFactsSection } from "@/presenters/review-plan"

/*
 * The one mark of a row of the tree of a review with its plan (spec/branch/review-plan.spec.md, In the Review Window):
 * details on demand. The row shows at most one small mark: in the warning tone when its file breaks uses left outside
 * the change, always; a muted one only on the row under the pointer, the selected and the focused row. Its hover opens
 * the card of the facts: what it breaks, its links among the changed files, the same edit as other files, and the
 * dependency changes the row carries. The presenter words them (`presentRowFacts`); a click on
 * a file in the card selects it and closes the card. The card opens and closes as every card of a row does
 * (`RowCard`): `I` on the focused row opens it with the focus on its first file, Escape gives the focus back. The tests
 * that name its code are those of its step (spec/branch/review-tour.spec.md).
 */

export interface FileRowMarkProps extends RowCardState {
  facts: RowFacts
  onSelectPlace: (place: ReviewPlace) => void
  /** The row is selected: a muted mark shows there as on the row under the pointer and the focused one */
  selected: boolean
}

/**
 * The mark of a row; its hover opens the card of its facts, and so does `I` on its row. Not a tab stop: the description
 * of the row says every fact, and the links are in the diffs and the overview of the plan too
 */
export function FileRowMark({ facts, onSelectPlace, selected, ...card }: FileRowMarkProps) {
  const warning = facts.tone === "warning"
  const Icon = warning ? TriangleAlertIcon : InfoIcon
  // the warning is the one mark a tree shows always; a muted one, on most rows of a change, only where the reader is
  // (the row under the pointer, the selected and the focused row, its card open), its place kept so no name moves
  const quiet = !warning && !selected && !card.open
  return (
    <RowCard
      {...card}
      label={facts.description}
      mark={
        <span
          className={cn(
            "my-0.5 flex size-4 shrink-0 items-center justify-center transition-opacity",
            warning ? "text-warning" : "text-muted-foreground",
            quiet && "opacity-0 group-hover/row:opacity-100 group-focus-visible/tree-item:opacity-100",
          )}
          data-quiet={quiet ? "" : undefined}
          data-testid="file-row-mark"
          data-tone={facts.tone}
        />
      }
      icon={<Icon className="size-3.5" />}
      className="w-80 p-3"
      data-testid="file-facts-card"
    >
      {close => (
        <FileFactsCard
          facts={facts}
          onSelectPlace={place => {
            close()
            onSelectPlace(place)
          }}
        />
      )}
    </RowCard>
  )
}

/** The sections of the card of a row's mark, each under its title; `onSelectPlace` shows another changed file */
export function FileFactsCard({ facts, onSelectPlace }: { facts: RowFacts; onSelectPlace: (place: ReviewPlace) => void }) {
  return (
    <>
      {facts.sections.map(section => (
        <div key={section.kind} className="flex flex-col gap-1" data-testid="file-facts-section" data-kind={section.kind}>
          <PopoverSectionTitle className="px-2">{section.title}</PopoverSectionTitle>
          <FactsSectionBody section={section} onSelectPlace={onSelectPlace} />
        </div>
      ))}
    </>
  )
}

function FactsSectionBody({ section, onSelectPlace }: { section: RowFactsSection; onSelectPlace: (place: ReviewPlace) => void }) {
  switch (section.kind) {
    case "breaks":
      return (
        <p className="px-2 text-xs" data-testid="file-facts-breaks" data-tone={section.tone}>
          <span className={cn("font-medium", section.tone === "warning" ? "text-warning" : "text-muted-foreground")}>{section.label}</span>
          <span className="text-muted-foreground">{` · ${section.why}`}</span>
        </p>
      )
    case "uses":
    case "usedBy":
      return <LinkList links={section.links} onSelectPlace={onSelectPlace} />
    case "sameEdit":
      return (
        <>
          <p className="truncate px-2 font-mono text-xs" title={section.edit} data-testid="file-same-edit">
            {section.edit}
          </p>
          {section.files.map(path => (
            <PopoverFile
              key={path}
              path={path}
              name={path.slice(path.lastIndexOf("/") + 1)}
              detail={path}
              testId="file-same-edit-file"
              onSelect={() => onSelectPlace(fileTop(path))}
            />
          ))}
        </>
      )
    case "dependencies":
      return (
        <span className="flex flex-col px-2 text-xs" data-testid="file-facts-dependencies">
          {section.lines.map((line, index) => (
            // two changes can read the same: the same name in two scopes of one module
            <span key={index} data-testid="file-facts-dependency">
              {line}
            </span>
          ))}
        </span>
      )
  }
}

/** Each link of a file: the other file, the names and the line of the first use, its state; a click shows the other file */
function LinkList({ links, onSelectPlace }: { links: readonly LinkEntry[]; onSelectPlace: (place: ReviewPlace) => void }) {
  return links.map(link => (
    <PopoverFile
      key={link.key}
      path={link.path}
      name={link.name}
      detail={link.symbols}
      aside={link.stateWord}
      state={link.state}
      testId="file-link"
      onSelect={() => onSelectPlace(link.place)}
    />
  ))
}

/** A changed file in the card: its name, what it says of it; a click selects it */
function PopoverFile({
  path,
  name,
  detail,
  aside,
  onSelect,
  testId,
  state,
}: {
  path: string
  name: string
  detail: string
  aside?: string
  onSelect: () => void
  testId: string
  state?: string
}) {
  return (
    <button
      type="button"
      className="flex w-full min-w-0 flex-col items-start gap-0.5 rounded-md px-2 py-1 text-left bd-focus-inset hover:bg-muted focus-visible:bd-focus"
      title={path}
      onClick={onSelect}
      data-testid={testId}
      data-path={path}
      data-state={state}
    >
      <span className="flex w-full min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1 truncate">{name}</span>
        {aside !== undefined && <span className="shrink-0 text-xs text-muted-foreground">{aside}</span>}
      </span>
      <span className="w-full min-w-0 truncate font-mono text-xs text-muted-foreground">{detail}</span>
    </button>
  )
}
