import { Fragment, useState } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { CommitBodyBlockView } from "@/components/commit/CommitMessage"
import type { BodyTextRenderer } from "@/components/commit/CommitMessage"
import { LinkedText } from "@/components/commit/LinkedText"
import { Callout } from "@/components/shared/Callout"
import type { PlacedBlock, StoryPart, StoryView } from "@/presenters/review-story"

/** The text of a line of a block of a story: its code and its issue links */
function storyText(_story: StoryView, _block: number): BodyTextRenderer {
  return text => <LinkedText text={text} detectCode />
}

/** A block of a story at its place in the body */
export function StoryBlock({ story, placed }: { story: StoryView; placed: PlacedBlock }) {
  return <CommitBodyBlockView block={placed.block} text={storyText(story, placed.index)} />
}

/**
 * The body of a commit as the review tells it (spec/branch/review-tour.spec.md, "In the Review Window"): its lead a
 * step larger, then its parts before the fold, a label the caption of its blocks and a Look closely of the author an
 * info callout; then one line that names the folded parts ("The cause · Tests · Evidence"), each its own disclosure,
 * an open one below the line, so a disclosure grows downward. Rust says where the fold is; a verification section is
 * folded always. The text is the foreground's: no part of the author's words is muted
 */
export function StoryBody({ story, className }: { story: StoryView; className?: string }) {
  const [opened, setOpened] = useState<ReadonlySet<string>>(() => new Set())
  const toggle = (key: string) =>
    setOpened(previous => {
      const next = new Set(previous)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })
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
          <StoryPartView key={part.key} story={story} part={part} />
        ))}
    </div>
  )
}

/** A part of a story: its label or heading above its blocks; a Look closely an info callout */
function StoryPartView({ story, part }: { story: StoryView; part: StoryPart }) {
  const blocks = part.blocks.map(placed => <StoryBlock key={placed.index} story={story} placed={placed} />)
  const head = part.head
  if (part.tone === "lookClosely" && head !== null) {
    return (
      <Callout tone="info" title={head.block.text} data-testid="review-story-look-closely" className="text-foreground">
        <div className="flex flex-col gap-2 text-foreground">{blocks}</div>
      </Callout>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-1" data-testid="review-story-part" data-part={part.key}>
      {head?.block.type === "label" && (
        <div data-slot="story-label" className="font-semibold">
          {head.block.text}
        </div>
      )}
      {head?.block.type === "heading" && (
        <div data-slot="story-heading" className="text-md font-semibold">
          {storyText(story, head.index)(head.block.text, 0)}
        </div>
      )}
      {blocks.length > 0 && <div className="flex min-w-0 flex-col gap-2">{blocks}</div>}
    </div>
  )
}
