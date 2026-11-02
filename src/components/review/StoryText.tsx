import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CodeText } from "@/components/shared/CodeText"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { CodeSpan } from "@/lib/bindings"
import { isCodeChip } from "@/lib/inline-code"
import type { StoryMention } from "@/presenters/review-story"

/**
 * A line of a block of a story (spec/branch/review-tour.spec.md, "Anchors"), its spans of code as Rust found them, the
 * same as a card sets (`CodeText`): a span that names what Rust resolved is a link in the code font, its dotted
 * underline the one mark of a link ("allowed — new · Manager.kt:6 · step 2" in its hover), a click showing it in the
 * diffs, its file selected and its name; a span that resolves nothing stays code; the prose between them keeps its
 * issue links. The ranges are UTF-16 offsets, the backticks inside them
 */
export function StoryText({ text, spans, mentions }: { text: string; spans: readonly CodeSpan[]; mentions: readonly StoryMention[] }) {
  const navigator = useSelectReviewFile()
  return (
    <CodeText
      text={text}
      spans={spans}
      code={(span, written) => {
        const mention = mentions.find(it => it.start === span.start && it.end === span.end)
        const place = mention?.place ?? null
        if (mention === undefined || place === null || mention.title === null) {
          return null
        }
        return (
          <Tooltip>
            <TooltipTrigger
              render={
                <button
                  type="button"
                  data-testid="review-story-mention"
                  data-path={place.path}
                  disabled={navigator === null}
                  data-explicit={span.explicit ? "" : undefined}
                  className={cn(
                    "rounded-xs font-mono leading-none [overflow-wrap:anywhere] underline decoration-muted-foreground/60 decoration-dotted underline-offset-3 outline-none hover:bg-accent hover:decoration-foreground hover:decoration-solid focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline",
                    isCodeChip(span) && "bg-muted ring-2 ring-muted",
                  )}
                  onClick={() => navigator?.show(place)}
                />
              }
            >
              {written}
            </TooltipTrigger>
            <TooltipContent data-testid="review-story-mention-tip">{mention.title}</TooltipContent>
          </Tooltip>
        )
      }}
    />
  )
}
