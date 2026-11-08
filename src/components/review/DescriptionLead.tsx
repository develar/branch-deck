import { ChevronRightIcon } from "lucide-react"
import { BodyPartsView } from "@/components/commit/CommitBodyView"
import { useBodyFold } from "@/hooks/useBodyFold"
import { useCommitBody } from "@/hooks/useCommitBody"
import { presentSummary } from "@/presenters/commit-body"

/**
 * The description of a pull request as the lead of the document of its changes (spec/repository/review-requests.spec.md,
 * "Review window"): its summary alone (`CommitBody.summary`, `presentSummary`), as the body view sets it, then
 * "Description", which shows the overview, where it stands whole; a description of its summary alone has no link, one
 * of no summary the link alone. `message` is the title and the body, read as a commit message
 */
export function DescriptionLead({ message, onShowDescription }: { message: string; onShowDescription: () => void }) {
  const body = useCommitBody(message)
  const fold = useBodyFold()
  if (body === undefined) {
    return null
  }
  const { parts, more } = presentSummary(body)
  return (
    <div className="flex min-w-0 flex-col gap-1.5" data-testid="review-description-lead">
      {parts !== null && <BodyPartsView parts={parts} size="reading" fold={fold} />}
      {more && (
        <button
          type="button"
          className="flex items-center gap-0.5 self-start rounded-sm text-sm text-muted-foreground outline-none hover:text-foreground focus-visible:bd-focus"
          title="Show the whole description in Overview"
          data-testid="review-description-link"
          onClick={onShowDescription}
        >
          Description
          <ChevronRightIcon aria-hidden className="size-3.5 shrink-0" />
        </button>
      )}
    </div>
  )
}
