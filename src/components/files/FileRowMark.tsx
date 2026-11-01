import { useCallback, useEffect, useEffectEvent, useRef } from "react"
import type { KeyboardEvent, MouseEvent } from "react"
import { cn } from "cn"
import { InfoIcon, TriangleAlertIcon } from "lucide-react"
import { PopoverContent, PopoverSectionTitle, PopoverTrigger } from "@/components/ui/popover"
import { HoverDescription } from "@/components/shared/HoverDescription"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { LinkEntry, RowFacts, RowFactsSection } from "@/presenters/review-plan"

/*
 * The one mark of a row of the tree of a review with its plan (spec/branch/review-plan.spec.md, In the Review Window):
 * details on demand. The row shows at most one small mark, in the warning tone when its file breaks uses left outside
 * the change; its hover opens the card of the facts: what it breaks, its links among the changed files, the same edit
 * as other files, and the dependency changes the row carries. The presenter words them (`presentRowFacts`); a click on
 * a file in the card selects it and closes the card. `I` on the focused row opens the card from the keyboard, the
 * focus on its first file; Escape closes it and gives the focus back to the row, and so does any other close that would
 * drop the focus to the page (the hover of another mark, the pointer leaving). The tests that name its code are those
 * of its step (spec/branch/review-tour.spec.md).
 */

/** A click on the mark or in its card acts there only: the row under it does not select or toggle */
function keepFromRow(event: MouseEvent) {
  event.stopPropagation()
}

/**
 * The first file of a card opened by the key takes the focus as it mounts, the card itself when it lists none (Breaks
 * alone), so its Escape is the card's; no scroll while the card is not placed yet
 */
function focusFirstFile(popup: HTMLElement | null) {
  const target = popup?.querySelector<HTMLElement>("button") ?? popup
  target?.focus({ preventScroll: true })
}

export interface FileRowMarkProps {
  facts: RowFacts
  onSelectPlace: (place: ReviewPlace) => void
  /** The tree keeps one card open: by the hover of the mark, or by `I` on the row */
  open: boolean
  /** Opened by `I` on the row: the focus moves into the card, and goes back to the row as the card closes by Escape or a file */
  openedByKey: boolean
  onOpenChange: (open: boolean) => void
  /** Gives the focus back to the row of the mark */
  focusRow: () => void
}

/**
 * The mark of a row; its hover opens the card of its facts (`HoverDescription`, after a move of the pointer), and so does
 * `I` on its row. Not a tab stop: the description of the row says every fact, and the links are in the diffs and the
 * overview of the plan too
 */
export function FileRowMark({ facts, onSelectPlace, open, openedByKey, onOpenChange, focusRow }: FileRowMarkProps) {
  const Icon = facts.tone === "warning" ? TriangleAlertIcon : InfoIcon
  // the popup of the card opened by the key, kept as the ref lets it go (before the effect below asks where the focus
  // is); stable, so a render does not focus its first file again
  const keyPopup = useRef<HTMLElement | null>(null)
  const mountKeyPopup = useCallback((popup: HTMLElement | null) => {
    if (popup !== null) {
      keyPopup.current = popup
      focusFirstFile(popup)
    }
  }, [])
  const focusBackIfLost = useEffectEvent(() => {
    const focused = document.activeElement
    // still in the card that goes, or fallen to the page as it went: a focus that went elsewhere on purpose stays
    if (focused === null || focused === document.body || keyPopup.current?.contains(focused) === true) {
      focusRow()
    }
  })
  // a card opened by the key that closes otherwise than by Escape or a file (the hover of another row's mark replaces
  // it, the pointer leaves its mark): the focus goes back to the row, else the next Escape would close the window
  useEffect(() => {
    if (!openedByKey) {
      return
    }
    return () => focusBackIfLost()
  }, [openedByKey])
  // the row goes (a search, a new view) or loses its mark: its card goes too, and does not open again as it comes back
  const closeAsMarkGoes = useEffectEvent(() => {
    if (open) {
      onOpenChange(false)
    }
  })
  useEffect(() => () => closeAsMarkGoes(), [])
  const close = (giveFocusBack: boolean) => {
    onOpenChange(false)
    if (giveFocusBack && openedByKey) {
      focusRow()
    }
  }
  // the Escape of the card is not the window's (Overview, Close)
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault()
      close(true)
    }
  }
  return (
    <HoverDescription
      lazy
      open={open}
      onOpenChange={(next, eventDetails) => {
        if (next) {
          onOpenChange(true)
        } else {
          // a click or the focus elsewhere keeps it where it went
          close(eventDetails.reason === "escape-key")
        }
      }}
    >
      <PopoverTrigger
        openOnHover
        delay={300}
        nativeButton={false}
        tabIndex={-1}
        aria-label={facts.description}
        render={
          <span
            className={cn("flex size-4 shrink-0 items-center justify-center", facts.tone === "warning" ? "text-warning" : "text-muted-foreground")}
            data-testid="file-row-mark"
            data-tone={facts.tone}
            onClick={keepFromRow}
          />
        }
      >
        <Icon className="size-3.5" />
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-80 gap-3 p-3"
        data-testid="file-facts-card"
        // the card moves the focus itself: by the key into it and back to the row, by the hover never
        initialFocus={false}
        finalFocus={false}
        ref={openedByKey ? mountKeyPopup : undefined}
        // the popup is in a portal, but React bubbles its events through the row: a click here must not toggle it
        onClick={keepFromRow}
        onContextMenu={keepFromRow}
        onKeyDown={handleKeyDown}
      >
        <FileFactsCard
          facts={facts}
          onSelectPlace={place => {
            close(true)
            onSelectPlace(place)
          }}
        />
      </PopoverContent>
    </HoverDescription>
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
      className="flex w-full min-w-0 flex-col items-start gap-0.5 rounded-md px-2 py-1 text-left hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
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
