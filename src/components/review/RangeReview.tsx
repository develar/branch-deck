import { useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitReview } from "@/components/commit/CommitReview"
import { CommitSubject } from "@/components/commit/CommitSubject"
import type { ReviewCommit } from "@/lib/bindings"
import { hasCommitMessageBody } from "@/lib/commit"
import type { DiffSubject } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { handlePreviewItemSwitchKey } from "@/lib/preview-keys"
import { isSingleCommit } from "@/lib/range-review"
import type { RangeReviewItem } from "@/lib/range-review"
import type { ReviewPosition } from "@/lib/review-position"

export interface RangeReviewProps {
  /** The scopes, then the commits newest first */
  items: readonly RangeReviewItem[]
  /** The key of the shown item */
  selected: string
  onSelect: (key: string) => void
  /** What the shown item diffs: the range of a scope, or the commit */
  subject: DiffSubject
  initialPath: string | null
  onPositionChange: (position: ReviewPosition) => void
  pathFilter?: ReadonlySet<string> | null
  filteredEmptyText?: string
  fileOwners?: ReadonlyMap<string, string>
  /** What the range is, in the error: "Cannot read the changes of the branch" */
  rangeNoun: string
}

/**
 * The review of a range, a pull request or a part of a stack: its commits beside the commit review, as the commit
 * picker of a code review on GitHub or in IntelliJ. A scope ("All Changes") diffs the range, a commit only itself, with
 * its subject, meta line and message above the diffs. ⌥↑/⌥↓ step through the column from anywhere in the review, ↑/↓
 * inside it. A range of one commit shows that commit without the column.
 */
export function RangeReview({ items, selected, onSelect, subject, initialPath, onPositionChange, pathFilter, filteredEmptyText, fileOwners, rangeNoun }: RangeReviewProps) {
  const single = isSingleCommit(items)
  const shown = single ? items.find(item => item.kind === "commit") : items.find(item => item.key === selected)
  const index = items.findIndex(item => item.key === selected)

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!event.defaultPrevented && !single) {
      handlePreviewItemSwitchKey(event, index, items.length, next => onSelect(items[next]!.key))
    }
  }

  return (
    <div className="flex h-full min-h-0" onKeyDown={handleKeyDown}>
      {!single && <CommitColumn items={items} selected={selected} onSelect={onSelect} />}
      <div className="flex min-w-0 flex-1 flex-col">
        {shown?.kind === "commit" && <ReviewCommitHeader key={shown.key} commit={shown.commit} />}
        <div className="min-h-0 flex-1">
          <CommitReview
            subject={subject}
            initialPath={initialPath}
            onPositionChange={onPositionChange}
            pathFilter={pathFilter}
            filteredEmptyText={filteredEmptyText}
            fileOwners={fileOwners}
            rangeNoun={rangeNoun}
          />
        </div>
      </div>
    </div>
  )
}

/** The commit column: one tab stop, ↑/↓ choose inside it, the choice told to a screen reader */
function CommitColumn({ items, selected, onSelect }: { items: readonly RangeReviewItem[]; selected: string; onSelect: (key: string) => void }) {
  const index = items.findIndex(item => item.key === selected)
  const shown = items[index]
  const commitCount = items.filter(item => item.kind === "commit").length

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.altKey || event.metaKey || event.ctrlKey || event.shiftKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) {
      return
    }
    event.preventDefault()
    const next = items[index + (event.key === "ArrowUp" ? -1 : 1)]
    if (next !== undefined) {
      onSelect(next.key)
    }
  }

  return (
    <div className="flex w-64 shrink-0 flex-col border-r border-border">
      <div
        role="listbox"
        aria-label="Commits"
        tabIndex={0}
        aria-activedescendant={shown === undefined ? undefined : optionId(shown.key)}
        className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2 outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
        data-testid="range-review-commits"
        onKeyDown={handleKeyDown}
      >
        {items.map(item => (
          <ColumnItem key={item.key} item={item} selected={item.key === selected} onSelect={onSelect} />
        ))}
      </div>
      <p aria-live="polite" className="sr-only">
        {shown?.kind === "commit" ? `Commit ${items.filter(item => item.kind === "commit").indexOf(shown) + 1} of ${commitCount}: ${shown.commit.subject}` : shown?.label}
      </p>
    </div>
  )
}

function ColumnItem({ item, selected, onSelect }: { item: RangeReviewItem; selected: boolean; onSelect: (key: string) => void }) {
  const option = (children: ReactNode, title?: string) => (
    <div
      id={optionId(item.key)}
      role="option"
      aria-selected={selected}
      title={title}
      data-testid="range-review-commit"
      className={cn("flex h-7 w-full shrink-0 cursor-default items-center gap-2 rounded-md px-2 text-left hover:bg-muted", selected && "bg-primary/15 hover:bg-primary/15")}
      // the column keeps the focus: a click chooses, the keys go on from there
      onMouseDown={event => event.preventDefault()}
      onClick={event => {
        event.currentTarget.closest<HTMLElement>("[role=listbox]")?.focus()
        onSelect(item.key)
      }}
    >
      {children}
    </div>
  )
  if (item.kind === "scope") {
    return option(
      <>
        <span className="flex-1 truncate font-medium">{item.label}</span>
        {item.detail !== undefined && <span className="text-xs text-muted-foreground">{item.detail}</span>}
      </>,
    )
  }
  return (
    <>
      {item.group !== undefined && (
        <p role="presentation" className="truncate px-2 pt-2 text-xs text-muted-foreground">
          {item.group}
        </p>
      )}
      {item.divider !== undefined && (
        <div role="presentation" className="flex items-center gap-2 px-2 pt-1 text-xs text-muted-foreground" data-testid="range-review-divider">
          <span className="h-px flex-1 bg-border" />
          {item.divider}
          <span className="h-px flex-1 bg-border" />
        </div>
      )}
      {option(
        <>
          <span className="flex-1 truncate">{item.commit.subject}</span>
          <span className="font-mono text-xs text-muted-foreground">{formatShortHash(item.commit.id)}</span>
        </>,
        item.commit.subject,
      )}
    </>
  )
}

function optionId(key: string): string {
  return `review-commit-${key}`
}

/** The shown commit above its diffs: the subject, the meta line and the message, three lines until Show More */
function ReviewCommitHeader({ commit }: { commit: ReviewCommit }) {
  const [expanded, setExpanded] = useState(false)
  const hasBody = hasCommitMessageBody(commit.subject, commit.message)
  return (
    <div className="flex shrink-0 flex-col gap-1 border-b border-border px-4 py-2" data-testid="range-review-commit-header">
      <CommitSubject subject={commit.subject} clamp={false} className="font-medium" />
      <CommitMetadata hash={commit.id} author={commit.author} authorTime={commit.authorTime} />
      {hasBody && (
        <div className="flex flex-col items-start gap-1">
          <div className={cn("max-h-[50vh] overflow-y-auto", !expanded && "line-clamp-3 overflow-hidden")}>
            <CommitMessageBody message={commit.message} />
          </div>
          <Button variant="link" size="xs" className="h-auto px-0" onClick={() => setExpanded(!expanded)}>
            {expanded ? "Show Less" : "Show More"}
          </Button>
        </div>
      )}
    </div>
  )
}
