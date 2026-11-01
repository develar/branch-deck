import { Fragment } from "react"
import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LinkedText } from "@/components/commit/LinkedText"
import { InlineCode } from "@/components/shared/InlineCode"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { StoryMention } from "@/presenters/review-story"

/**
 * A line of a block of a story (spec/branch/review-tour.spec.md, "Anchors"), cut at its code names: a name that
 * resolves is a link in the code font, its hover names what it is ("allowed — new · Manager.kt:6 · step 2"), a click
 * shows it in the diffs, its file selected and its name; a name that resolves nothing stays code; the text between
 * them is prose with its code and its issue links. The ranges are UTF-16 offsets, the backticks inside them
 */
export function StoryText({ text, mentions }: { text: string; mentions: readonly StoryMention[] }) {
  const navigator = useSelectReviewFile()
  if (mentions.length === 0) {
    return <LinkedText text={text} detectCode />
  }
  const parts = []
  let at = 0
  for (const mention of mentions.toSorted((a, b) => a.start - b.start)) {
    if (mention.start < at || mention.end > text.length) {
      continue
    }
    if (mention.start > at) {
      parts.push(<LinkedText key={`text:${at}`} text={text.slice(at, mention.start)} detectCode />)
    }
    const source = text.slice(mention.start, mention.end)
    // a name in backticks is a chip, a bare name only the code font, as the rest of the text sets them
    const explicit = source.startsWith("`")
    const written = explicit ? source.replace(/^`|`$/g, "") : source
    const place = mention.place
    parts.push(
      place === null || mention.title === null ? (
        <InlineCode key={`code:${mention.start}`} data-explicit="">
          {written}
        </InlineCode>
      ) : (
        <Tooltip key={`link:${mention.start}`}>
          <TooltipTrigger
            render={
              <button
                type="button"
                data-testid="review-story-mention"
                data-path={place.path}
                disabled={navigator === null}
                data-explicit={explicit ? "" : undefined}
                className={cn(
                  "rounded-sm font-mono leading-none [overflow-wrap:anywhere] underline decoration-muted-foreground/60 decoration-dotted underline-offset-3 outline-none hover:bg-accent hover:decoration-foreground hover:decoration-solid focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline",
                  explicit && "bg-muted px-1",
                )}
                onClick={() => navigator?.show(place)}
              />
            }
          >
            {written}
          </TooltipTrigger>
          <TooltipContent data-testid="review-story-mention-tip">{mention.title}</TooltipContent>
        </Tooltip>
      ),
    )
    at = mention.end
  }
  if (at < text.length) {
    parts.push(<LinkedText key={`text:${at}`} text={text.slice(at)} detectCode />)
  }
  return <Fragment>{parts}</Fragment>
}
