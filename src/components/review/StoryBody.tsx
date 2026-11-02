import { Fragment, useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { CommitBodyBlockView } from "@/components/commit/CommitBodyView"
import type { BodyTextRenderer } from "@/components/commit/CommitBodyView"
import { StoryText } from "@/components/review/StoryText"
import { Callout } from "@/components/shared/Callout"
import { useStoryHighlight } from "@/hooks/useStoryHighlight"
import type { StoryHighlight } from "@/hooks/useStoryHighlight"
import type { PlacedBlock, StoryPart, StoryView } from "@/presenters/review-story"

/** The text of a line of a block of a story: its code names links, its code and its issue links */
function storyText(story: StoryView, block: number): BodyTextRenderer {
  return (text, item) => <StoryText text={text} mentions={story.mentions.filter(mention => mention.block === block && mention.item === item)} />
}

/** The one line of a label or a heading of a story, its code names links */
export function StoryLine({ story, block, text }: { story: StoryView; block: number; text: string }) {
  return storyText(story, block)(text, 0)
}

/**
 * The block a mark of a section showed once this story mounted (`useStoryHighlight`): a story that mounts later (the
 * subject shown again, a search that hid every file cleared) never plays a click before it again. `revealed` is the
 * click that opened the fold of the block, which mounts with it
 */
function useFreshHighlight(revealed: number | null = null): StoryHighlight | null {
  const highlight = useStoryHighlight()
  const [mounted] = useState(() => highlight?.serial ?? null)
  return highlight === null || (highlight.serial === mounted && highlight.serial !== revealed) ? null : highlight
}

interface StoryPlaceProps {
  readonly story: StoryView
  /** A copy of a block told elsewhere (the lead of a step in the contents of the overview) is no `told` place: no mark shows it */
  readonly told?: boolean
  /** The click that opened the fold the block is in */
  readonly revealed?: number | null
}

/**
 * A block of a story at its place in the body (`data-story-block`), a paragraph, a list or the label or heading of a
 * part, outlined for a moment when a mark of a section of the diffs showed it: its key changes with the click, so the
 * outline plays again
 */
function StoryAnchor({ story, index, told = true, revealed = null, children }: StoryPlaceProps & { index: number; children: ReactNode }) {
  const highlight = useFreshHighlight(revealed)
  const shown = told && highlight !== null && highlight.commit === story.commit && highlight.block === index ? highlight.serial : null
  const element = useRef<HTMLDivElement>(null)
  // the opening of its step is at the top: a block further down comes into view
  useEffect(() => {
    if (shown !== null) {
      element.current?.scrollIntoView({ block: "nearest" })
    }
  }, [shown])
  return (
    <div
      ref={element}
      key={shown ?? 0}
      data-story-block={told ? `${story.commit}:${index}` : undefined}
      data-highlight={shown === null ? undefined : ""}
      className="min-w-0 rounded-sm outline-2 outline-offset-4 outline-transparent data-highlight:animate-story-highlight"
    >
      {children}
    </div>
  )
}

/** A block of a story at its place in the body (`StoryAnchor`) */
export function StoryBlock({ placed, ...props }: StoryPlaceProps & { placed: PlacedBlock }) {
  return (
    <StoryAnchor index={placed.index} {...props}>
      <CommitBodyBlockView block={placed.block} text={storyText(props.story, placed.index)} />
    </StoryAnchor>
  )
}

/**
 * The body of a commit as the review tells it (spec/branch/review-tour.spec.md, "In the Review Window"): its lead a
 * step larger, then its parts before the fold, a label the caption of its blocks and a Look closely of the author an
 * info callout; then one line that names the folded parts ("The cause · Tests · Evidence"), each its own disclosure,
 * an open one below the line, so a disclosure grows downward. Rust says which part folds (`CommitBody.parts`): a
 * verification section always, the author's Look closely never. The text is the foreground's: no part of the author's
 * words is muted
 */
export function StoryBody({ story, className }: { story: StoryView; className?: string }) {
  const [opened, setOpened] = useState<ReadonlySet<string>>(() => new Set())
  // a mark of a section of the diffs showed a block a fold holds: its part opens, its blocks outlined as they mount
  const highlight = useFreshHighlight()
  const [seen, setSeen] = useState<number | null>(null)
  const [revealed, setRevealed] = useState<{ readonly key: string; readonly serial: number } | null>(null)
  if (highlight !== null && highlight.commit === story.commit && highlight.serial !== seen) {
    setSeen(highlight.serial)
    const folded = story.folded.find(({ part }) => part.head?.index === highlight.block || part.blocks.some(placed => placed.index === highlight.block))
    if (folded !== undefined && !opened.has(folded.part.key)) {
      setOpened(new Set([...opened, folded.part.key]))
      setRevealed({ key: folded.part.key, serial: highlight.serial })
    }
  }
  const toggle = (key: string) => {
    // a part opened again by hand plays no click
    setRevealed(null)
    setOpened(previous => {
      const next = new Set(previous)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })
  }
  return (
    <div
      data-testid="review-story"
      data-commit={story.commit}
      // ss04: distinct I, l and 1 in identifiers, as the commit preview sets them
      className={cn("flex max-w-prose cursor-text flex-col gap-3 [font-feature-settings:'ss04'] text-sm text-foreground select-text", className)}
    >
      {story.lead !== null && (
        <div data-testid="review-story-lead" className="text-md">
          <StoryBlock story={story} placed={story.lead} />
        </div>
      )}
      {story.shown.map(part => (
        <StoryPartView key={part.key} story={story} part={part} />
      ))}
      {story.folded.length > 0 && (
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-1 text-xs" data-testid="review-story-folded">
          {story.folded.map(({ part, name }, index) => (
            <Fragment key={part.key}>
              {index > 0 && (
                <span aria-hidden className="text-muted-foreground">
                  ·
                </span>
              )}
              <button
                type="button"
                aria-expanded={opened.has(part.key)}
                data-testid="review-story-fold"
                className="group flex items-center gap-0.5 rounded-sm font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30 aria-expanded:text-foreground"
                onClick={() => toggle(part.key)}
              >
                <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 transition-transform group-aria-expanded:rotate-90" />
                {name}
              </button>
            </Fragment>
          ))}
        </div>
      )}
      {story.folded
        .filter(({ part }) => opened.has(part.key))
        .map(({ part }) => (
          <StoryPartView key={part.key} story={story} part={part} revealed={revealed?.key === part.key ? revealed.serial : null} />
        ))}
    </div>
  )
}

/** A part of a story: its label or heading above its blocks; a Look closely an info callout. A code name in a label or heading is a link too */
function StoryPartView({ story, part, revealed = null }: { story: StoryView; part: StoryPart; revealed?: number | null }) {
  const blocks = part.blocks.map(placed => <StoryBlock key={placed.index} story={story} placed={placed} revealed={revealed} />)
  const head = part.head
  if (part.tone === "lookClosely" && head !== null) {
    return (
      <StoryAnchor story={story} index={head.index} revealed={revealed}>
        <Callout tone="info" title={<StoryLine story={story} block={head.index} text={head.block.text} />} data-testid="review-story-look-closely" className="text-foreground">
          <div className="flex flex-col gap-2 text-foreground">{blocks}</div>
        </Callout>
      </StoryAnchor>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-1" data-testid="review-story-part" data-part={part.key}>
      {(head?.block.type === "label" || head?.block.type === "heading") && (
        <StoryAnchor story={story} index={head.index} revealed={revealed}>
          <div data-slot={head.block.type === "label" ? "story-label" : "story-heading"} className={cn("font-semibold", head.block.type === "heading" && "text-md")}>
            <StoryLine story={story} block={head.index} text={head.block.text} />
          </div>
        </StoryAnchor>
      )}
      {blocks.length > 0 && <div className="flex min-w-0 flex-col gap-2">{blocks}</div>}
    </div>
  )
}
