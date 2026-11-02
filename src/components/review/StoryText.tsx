import { cloneElement } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { CodeText } from "@/components/shared/CodeText"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { CodeSpan } from "@/lib/bindings"
import { isCodeChip } from "@/lib/inline-code"
import type { StoryMention } from "@/presenters/review-story"

/**
 * A line of a block of a story (spec/branch/review-tour.spec.md, "Anchors"), its spans of code as Rust found them, the
 * same as a card sets (`CodeText`): a span that names what Rust resolved is a link in the code font, its dotted
 * underline the one mark of a link, a click showing it in the diffs, its file selected and its name; its hover a card
 * of what it names (`MentionCard`: the status, the file and line, another step), never its name again; inside another
 * card (`nested`, the quote of a story mark) only the link. A span that resolves nothing stays code; the prose between
 * them keeps its issue links. The ranges are UTF-16 offsets, the backticks inside them
 */
export function StoryText({ text, spans, mentions, nested = false }: { text: string; spans: readonly CodeSpan[]; mentions: readonly StoryMention[]; nested?: boolean }) {
  const navigator = useSelectReviewFile()
  return (
    <CodeText
      text={text}
      spans={spans}
      code={(span, written) => {
        const mention = mentions.find(it => it.start === span.start && it.end === span.end)
        const place = mention?.place ?? null
        if (mention === undefined || place === null || mention.card === null) {
          return null
        }
        const link = (
          <button
            type="button"
            data-testid="review-story-mention"
            data-path={place.path}
            // its name is the name it shows; what it names is its description, as its card says it
            aria-description={mention.description ?? undefined}
            disabled={navigator === null}
            data-explicit={span.explicit ? "" : undefined}
            className={cn(
              "rounded-xs font-mono leading-none [overflow-wrap:anywhere] underline decoration-muted-foreground/60 decoration-dotted underline-offset-3 outline-none hover:bg-accent hover:decoration-foreground hover:decoration-solid focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline",
              isCodeChip(span) && "bg-muted ring-2 ring-muted",
            )}
            onClick={() => navigator?.show(place)}
          />
        )
        if (nested) {
          return cloneElement(link, {}, written)
        }
        return (
          <HoverDescription lazy>
            <PopoverTrigger openOnHover delay={300} nativeButton render={link}>
              {written}
            </PopoverTrigger>
            <PopoverContent
              side="top"
              collisionPadding={8}
              className="w-auto max-w-sm flex-row items-center gap-2 px-2.5 py-1.5 text-xs"
              data-testid="review-story-mention-tip"
              initialFocus={false}
              finalFocus={false}
            >
              {mention.card.status !== null && <Badge variant={mention.card.tone}>{mention.card.status}</Badge>}
              <span className="font-mono text-muted-foreground">{mention.card.location}</span>
              {mention.card.step !== null && <span className="text-muted-foreground">{mention.card.step}</span>}
            </PopoverContent>
          </HoverDescription>
        )
      }}
    />
  )
}
