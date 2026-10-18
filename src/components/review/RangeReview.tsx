import { useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { CommitHeading } from "@/components/commit/CommitHeading"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitReview } from "@/components/commit/CommitReview"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
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
  /** The header of the files: the owner chips of a branch review */
  filesHeader?: ReactNode
}

/**
 * The review of a range, a pull request or a part of a stack: its commits above the files in the sidebar of the commit
 * review, as the commit picker of a code review on GitHub or in IntelliJ. A scope ("All Changes") diffs the range, a
 * commit only itself, with its subject, meta line and message above the diffs. ⌥↑/⌥↓ step through the commits from
 * anywhere in the review, ↑/↓ inside their list. A range of one commit shows that commit without the list.
 */
export function RangeReview({
  items,
  selected,
  onSelect,
  subject,
  initialPath,
  onPositionChange,
  pathFilter,
  filteredEmptyText,
  fileOwners,
  rangeNoun,
  filesHeader,
}: RangeReviewProps) {
  const single = isSingleCommit(items)
  const shown = single ? items.find(item => item.kind === "commit") : items.find(item => item.key === selected)
  const index = items.findIndex(item => item.key === selected)
  // the commits ⌥↑/⌥↓ show next
  const neighbours = single
    ? undefined
    : [items[index - 1], items[index + 1]].flatMap((item): DiffSubject[] => (item?.kind === "commit" ? [{ kind: "commit", id: item.commit.id }] : []))

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!event.defaultPrevented && !single) {
      handlePreviewItemSwitchKey(event, index, items.length, next => onSelect(items[next]!.key))
    }
  }

  return (
    <div className="h-full min-h-0" onKeyDown={handleKeyDown}>
      <CommitReview
        subject={subject}
        initialPath={initialPath}
        onPositionChange={onPositionChange}
        pathFilter={pathFilter}
        filteredEmptyText={filteredEmptyText}
        fileOwners={fileOwners}
        rangeNoun={rangeNoun}
        neighbours={neighbours}
        sidebarTop={single ? undefined : <CommitColumn items={items} selected={selected} onSelect={onSelect} />}
        filesHeader={filesHeader}
        codeTop={shown?.kind === "commit" ? <ReviewCommitHeader key={shown.key} commit={shown.commit} /> : undefined}
      />
    </div>
  )
}

/** The commits of the range above the files: one tab stop, ↑/↓ choose inside it, the choice told to a screen reader */
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
    <>
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
    </>
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
        <div role="presentation" className="px-2 pt-2">
          <SectionLabel title={item.group} />
        </div>
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

/** The shown commit above its diffs, as the branch preview heads a commit (`CommitHeading`); the message three lines until Show More */
function ReviewCommitHeader({ commit }: { commit: ReviewCommit }) {
  const [expanded, setExpanded] = useState(false)
  const fileStats = useCommitFileStats(commit.id)
  const hasBody = hasCommitMessageBody(commit.subject, commit.message)
  return (
    <div className="shrink-0 border-b border-border px-4 py-3" data-testid="range-review-commit-header">
      <CommitHeading subject={commit.subject} hash={commit.id} author={commit.author} authorTime={commit.authorTime} committerTime={commit.committerTime} fileStats={fileStats}>
        {hasBody && (
          <div className="flex flex-col items-start gap-1">
            <div className={cn("max-h-[50vh] overflow-y-auto", !expanded && "line-clamp-3 overflow-hidden")}>
              <CommitMessageBody message={commit.message} size="reading" />
            </div>
            <Button variant="link" size="xs" className="h-auto px-0" onClick={() => setExpanded(!expanded)}>
              {expanded ? "Show Less" : "Show More"}
            </Button>
          </div>
        )}
      </CommitHeading>
    </div>
  )
}
