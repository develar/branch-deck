import { useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { BodyPartsView, CommitBodyBlockView } from "@/components/commit/CommitBodyView"
import type { BodyTextRenderer } from "@/components/commit/CommitBodyView"
import { StoryText } from "@/components/review/StoryText"
import { useBodyFold } from "@/hooks/useBodyFold"
import { useStoryHighlight } from "@/hooks/useStoryHighlight"
import type { StoryHighlight } from "@/hooks/useStoryHighlight"
import type { PlacedBlock } from "@/presenters/commit-body"
import type { StoryView } from "@/presenters/review-story"

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
 * The body of a commit as the review tells it (spec/branch/review-tour.spec.md, "In the Review Window"): the one view of
 * a body (`BodyPartsView`) at the size of a story, its lead a step larger, no trailer; its code names links, each block
 * at its place (`StoryAnchor`), a part a mark of a section of the diffs showed opened
 */
export function StoryBody({ story }: { story: StoryView }) {
  const fold = useBodyFold()
  // a mark of a section of the diffs showed a block a fold holds: its part opens, its blocks outlined as they mount
  const highlight = useFreshHighlight()
  const [seen, setSeen] = useState<number | null>(null)
  if (highlight !== null && highlight.commit === story.commit && highlight.serial !== seen) {
    setSeen(highlight.serial)
    const folded = story.folded.find(({ part }) => part.head?.index === highlight.block || part.blocks.some(placed => placed.index === highlight.block))
    if (folded !== undefined) {
      fold.reveal(folded.part.key, highlight.serial)
    }
  }
  return (
    <BodyPartsView
      parts={story}
      size="story"
      fold={fold}
      rendering={{
        text: index => storyText(story, index),
        place: (index, revealedBy, node) => (
          <StoryAnchor story={story} index={index} revealed={revealedBy}>
            {node}
          </StoryAnchor>
        ),
      }}
      testId="review-story"
      commit={story.commit}
    />
  )
}
