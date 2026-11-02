import type { KeyboardEvent, ReactElement, ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { CommitReview } from "@/components/commit/CommitReview"
import type { CommitReviewProps } from "@/components/commit/CommitReview"
import { ReviewPrologue } from "@/components/review/ReviewPrologue"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { LazyContextMenu } from "@/components/shared/LazyContextMenu"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { ReviewCommit } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import { handlePreviewItemSwitchKey } from "@/lib/preview-keys"
import { isSingleCommit, storyCommits } from "@/lib/range-review"
import type { RangeReviewItem } from "@/lib/range-review"
import type { ReviewPlanState } from "@/lib/review-plan-answer"
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
  /** The files of the chosen owner chip, marked in the whole story */
  ownerMarks?: CommitReviewProps["ownerMarks"]
  fileOwners?: ReadonlyMap<string, string>
  /** What the range is, in the error: "Cannot read the changes of the branch" */
  rangeNoun: string
  /** The header of the files: the owner chips of a branch review */
  filesHeader?: CommitReviewProps["filesHeader"]
  /** At the end of the heading of the shown commit: "Review bazel" of a commit of the whole stack */
  commitAction?: (item: Extract<RangeReviewItem, { kind: "commit" }>) => ReactNode
  /** The menu of a commit: its right-click in the column and the "⋯" of its heading; none without */
  commitMenu?: (commit: ReviewCommit) => MenuItemGroups
  /** The read of the review plan of what is shown: its overview heads the document; none without the plan */
  plan?: ReviewPlanState | null
  /** The facts of the review plan of the scope: the badges of the files, the Mechanical group */
  reviewFacts?: CommitReviewProps["reviewFacts"]
  /** The overview names what is reviewed; false where a title above names it already (the island of a pull request) */
  planHeadline?: boolean
}

/**
 * The review of a range, a pull request or a part of a stack: its commits above the files in the sidebar of the commit
 * review, as the commit picker of a code review on GitHub or in IntelliJ. A scope ("All Changes") diffs the range, a
 * commit only itself, with its subject, meta line and whole message at the top of its document. ⌥↑/⌥↓ step through the commits from
 * anywhere in the review, ↑/↓ inside their list. A range of one commit shows that commit without the list. With the
 * plan, a scope lists its commits in the steps of its tree (spec/branch/review-tour.spec.md, "In the Review Window"),
 * and the column only the scopes: a commit row shows its step, Enter and Show Commit Alone the commit alone. The tree
 * lists the commits of the scope only; while it lists none (no tree, a search that keeps no step) the column has them.
 */
export function RangeReview({
  items,
  selected,
  onSelect,
  subject,
  initialPath,
  onPositionChange,
  ownerMarks,
  fileOwners,
  rangeNoun,
  filesHeader,
  commitAction,
  commitMenu,
  plan = null,
  reviewFacts,
  planHeadline = true,
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
  // the story lists the commits of a scope in its steps: the column keeps the scopes
  const inStory = !single && shown?.kind === "scope" && reviewFacts != null
  const commitRows: CommitReviewProps["commitRows"] = inStory
    ? {
        items: storyCommits(items, shown.key),
        onShow: onSelect,
        menu: item => [[{ id: "show-commit-alone", label: "Show Commit Alone", onSelect: () => onSelect(item.key) }], ...(commitMenu?.(item.commit) ?? [])],
      }
    : null

  return (
    <div className="h-full min-h-0" onKeyDown={handleKeyDown}>
      <CommitReview
        subject={subject}
        initialPath={initialPath}
        onPositionChange={onPositionChange}
        ownerMarks={ownerMarks}
        fileOwners={fileOwners}
        rangeNoun={rangeNoun}
        neighbours={neighbours}
        sidebarTop={
          single
            ? undefined
            : treeListsCommits => (
                <CommitColumn
                  items={inStory && treeListsCommits ? items.filter(item => item.kind === "scope") : items}
                  selected={selected}
                  onSelect={onSelect}
                  commitMenu={commitMenu}
                />
              )
        }
        commitRows={commitRows}
        filesHeader={filesHeader}
        reviewFacts={reviewFacts}
        prologue={
          shown?.kind === "commit" ? (
            <ReviewPrologue
              key={shown.key}
              state={plan}
              commit={{
                commit: shown.commit,
                action: (
                  <>
                    {commitAction?.(shown)}
                    {commitMenu !== undefined && <CommitHeadingMenu commit={shown.commit} commitMenu={commitMenu} />}
                  </>
                ),
              }}
            />
          ) : plan === null ? undefined : (
            <ReviewPrologue state={plan} headline={planHeadline} />
          )
        }
      />
    </div>
  )
}

/** The commits of the range above the files: one tab stop, ↑/↓ choose inside it, the choice told to a screen reader */
function CommitColumn({
  items,
  selected,
  onSelect,
  commitMenu,
}: {
  items: readonly RangeReviewItem[]
  selected: string
  onSelect: (key: string) => void
  commitMenu: RangeReviewProps["commitMenu"]
}) {
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
          <ColumnItem key={item.key} item={item} selected={item.key === selected} onSelect={onSelect} commitMenu={commitMenu} />
        ))}
      </div>
      <p aria-live="polite" className="sr-only">
        {shown?.kind === "commit" ? `Commit ${items.filter(item => item.kind === "commit").indexOf(shown) + 1} of ${commitCount}: ${shown.commit.subject}` : shown?.label}
      </p>
    </>
  )
}

function ColumnItem({
  item,
  selected,
  onSelect,
  commitMenu,
}: {
  item: RangeReviewItem
  selected: boolean
  onSelect: (key: string) => void
  commitMenu: RangeReviewProps["commitMenu"]
}) {
  const option = (children: ReactNode, title?: string) => (
    <div
      id={optionId(item.key)}
      role="option"
      aria-selected={selected}
      title={title}
      data-testid="range-review-commit"
      className={cn(
        "group/commit flex h-7 w-full shrink-0 cursor-default items-center gap-2 rounded-md px-2 text-left hover:bg-muted",
        selected && "bg-primary/15 hover:bg-primary/15",
      )}
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
      {withCommitMenu(
        item.commit,
        commitMenu,
        option(
          <>
            <span className="flex-1 truncate">{item.commit.subject}</span>
            {item.since !== undefined && (
              <Badge variant="info" data-testid="range-review-commit-since">
                {item.since}
              </Badge>
            )}
            {item.branch !== undefined && (
              <span className="max-w-24 shrink-0 truncate text-xs text-muted-foreground" data-testid="range-review-commit-branch">
                {item.branch}
              </span>
            )}
            {/* the subject gets the width: the hash shows on hover, and the heading of the shown commit has it */}
            <span className="hidden font-mono text-xs text-muted-foreground group-hover/commit:inline" data-testid="range-review-commit-hash">
              {formatShortHash(item.commit.id)}
            </span>
          </>,
          item.commit.subject,
        ),
      )}
    </>
  )
}

/** The commit of the column with its menu on the right-click, built when it opens; as it is without one */
function withCommitMenu(commit: ReviewCommit, commitMenu: RangeReviewProps["commitMenu"], option: ReactElement): ReactNode {
  if (commitMenu === undefined) {
    return option
  }
  return <LazyContextMenu menu={() => commitMenu(commit)} render={option} />
}

/** "⋯" at the end of the heading of the shown commit: the menu its right-click opens in the column */
function CommitHeadingMenu({ commit, commitMenu }: { commit: ReviewCommit; commitMenu: NonNullable<RangeReviewProps["commitMenu"]> }) {
  return <MoreActionsMenu name={formatShortHash(commit.id)} menu={<DropdownMenuItems groups={commitMenu(commit)} />} />
}

function optionId(key: string): string {
  return `review-commit-${key}`
}
