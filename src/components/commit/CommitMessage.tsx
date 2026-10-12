import type { MouseEvent } from "react"
import { RectangleEllipsisIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { LinkedText } from "@/components/commit/LinkedText"
import { getCommitMessageBody } from "@/lib/commit"
import { fitsGutter, groupCommitBodySections, parseCommitBody } from "@/lib/commit-message"
import type { CommitBodyBlock, CommitBodySection } from "@/lib/commit-message"

export interface CommitMessageToggleProps {
  expanded: boolean
  onToggle: () => void
}

/** Icon button that shows or hides the full commit message inline; does not change the row selection. */
export function CommitMessageToggle({ expanded, onToggle }: CommitMessageToggleProps) {
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-expanded={expanded}
      aria-label={expanded ? "Hide full commit message" : "Show full commit message"}
      data-testid="commit-message-toggle"
      // `-my-0.5`: the 24px button keeps the 20px line height of the subject
      className="-my-0.5 shrink-0 text-muted-foreground hover:text-foreground aria-expanded:text-foreground"
      onClick={(event: MouseEvent) => {
        event.stopPropagation()
        onToggle()
      }}
    >
      <RectangleEllipsisIcon />
    </Button>
  )
}

/**
 * Body of a commit message (the message without its subject line), rendered only while expanded,
 * so rows don't parse their bodies.
 *
 * Lead-in labels ("Why:", "What changed:") go into a side gutter when the panel is wide enough; a label wider than the
 * gutter takes its own line above its blocks, as a man page `.TP` tag does. Code tokens are set in the monospace font.
 * The text is selectable, and a click in it does not change the row selection.
 */
export function CommitMessageBody({ message }: { message: string }) {
  const sections = groupCommitBodySections(parseCommitBody(getCommitMessageBody(message)))
  const hasLabels = sections.some(section => section.label != null)
  return (
    <div
      data-testid="commit-message-body"
      className="@container cursor-text rounded-lg bg-card px-4 py-3 ring-1 ring-border select-text"
      onClick={event => event.stopPropagation()}
    >
      <div
        className={cn(
          // ss04: distinct I, l and 1 in identifiers, without ss02's slashed zero in prose numbers
          "flex flex-col gap-4 text-[13px] leading-5 text-pretty text-foreground/85 [font-feature-settings:'ss04']",
          hasLabels ? "@xl:grid @xl:grid-cols-[8.5rem_minmax(0,54ch)]" : "max-w-[54ch]",
        )}
      >
        {/* sections of the same message never reorder, so the index is a stable key */}
        {sections.map((section, index) => <CommitBodySectionView key={index} section={section} inGrid={hasLabels} />)}
      </div>
    </div>
  )
}

/** `inGrid`: the body has labels, so on a wide panel the section spans the gutter and the text column of the body grid */
function CommitBodySectionView({ section, inGrid }: { section: CommitBodySection, inGrid: boolean }) {
  const { label } = section
  return (
    <div className={cn("flex flex-col gap-2", inGrid && "@xl:col-span-2 @xl:grid @xl:grid-cols-subgrid")}>
      {label != null && (
        <div data-slot="commit-section-label" className={cn("font-medium text-muted-foreground", !fitsGutter(label) && "@xl:col-span-2")}>
          {label}
        </div>
      )}
      <div className={cn("flex min-w-0 flex-col gap-2", inGrid && "@xl:col-start-2")}>
        {section.blocks.map((block, index) => <CommitBodyBlockView key={index} block={block} />)}
      </div>
    </div>
  )
}

function CommitBodyBlockView({ block }: { block: CommitBodyBlock }) {
  switch (block.type) {
    case "paragraph":
      return (
        <p>
          <LinkedText text={block.text} detectCode />
        </p>
      )
    case "list": {
      const items = block.items.map((item, index) => (
        <li key={index}>
          <LinkedText text={item} detectCode />
        </li>
      ))
      const className = "flex flex-col gap-1 pl-5 marker:text-muted-foreground"
      return block.ordered
        ? <ol start={block.start === 1 ? undefined : block.start} className={`list-decimal ${className}`}>{items}</ol>
        : <ul className={`list-disc ${className}`}>{items}</ul>
    }
    case "preformatted":
      return (
        <pre className="overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre">
          <LinkedText text={block.text} />
        </pre>
      )
    case "trailers":
      return (
        <div className="flex flex-col gap-0.5 border-t border-border pt-3 text-xs text-muted-foreground">
          {block.lines.map((line, index) => <LinkedText key={index} text={line} />)}
        </div>
      )
  }
}
