import { TriangleAlertIcon } from "lucide-react"
import { InlineCode } from "@/components/shared/InlineCode"

/** A short hash (`formatShortHash`) in the sentence of a notice; the sentences have no other 8-letter hex word */
const SHORT_HASH = /\b([0-9a-f]{8})\b/

/** The sentence with each short hash in the monospace font, as a hash shows everywhere */
function NoticeText({ text }: { text: string }) {
  return text.split(SHORT_HASH).map((part, index) =>
    index % 2 === 1 ? (
      <InlineCode key={index} plain>
        {part}
      </InlineCode>
    ) : (
      part
    ),
  )
}

/**
 * What the details of a review chip (Patronus, pull request) say about the commit the review took: a `warning` notice
 * when the branch changed since (`<testId>-outdated`), a quiet line when it was only rebuilt on a newer baseline
 * (`<testId>-rebased`), nothing without a text.
 */
export function ReviewCommitNotice({ text, outdated, testId }: { text: string | null; outdated: boolean; testId: string }) {
  if (text === null) {
    return null
  }
  if (!outdated) {
    return (
      <p className="text-xs text-muted-foreground" data-testid={`${testId}-rebased`}>
        <NoticeText text={text} />
      </p>
    )
  }
  return (
    <p className="flex items-start gap-1.5 text-xs text-warning" data-testid={`${testId}-outdated`}>
      <TriangleAlertIcon className="mt-px size-3.5 shrink-0" />
      <span>
        <NoticeText text={text} />
      </span>
    </p>
  )
}
