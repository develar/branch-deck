import { TriangleAlertIcon } from "lucide-react"

/**
 * What the details of a review chip (Patronus, pull request) say about the commit the review took: a `warning` notice
 * when the branch changed since (`<testId>-outdated`), a quiet line when it was only rebuilt on a newer baseline
 * (`<testId>-rebased`), nothing without a text.
 */
export function ReviewCommitNotice({ text, outdated, testId }: { text: string | null, outdated: boolean, testId: string }) {
  if (text === null) {
    return null
  }
  if (!outdated) {
    return <p className="text-xs text-muted-foreground" data-testid={`${testId}-rebased`}>{text}</p>
  }
  return (
    <p className="flex items-start gap-1.5 text-xs text-warning" data-testid={`${testId}-outdated`}>
      <TriangleAlertIcon className="mt-px size-3.5 shrink-0" />
      {text}
    </p>
  )
}
