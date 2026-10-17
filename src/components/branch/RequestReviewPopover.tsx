import { lazy, Suspense, useState } from "react"
import type { ReactElement } from "react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import type { PullRequest, ReviewReadiness } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"

const loadRequestReviewForm = () => import("@/components/branch/RequestReviewForm")
const RequestReviewForm = lazy(() => loadRequestReviewForm().then(it => ({ default: it.RequestReviewForm })))

export interface RequestReviewPopoverProps {
  branch: Branch
  /** The open pull request of the branch */
  pullRequest: PullRequest
  /** The review readiness of the branch: its missing groups name the code owners offered; null while unknown */
  readiness: ReviewReadiness | null
  /** What opens it, rendered by the trigger (Base UI `render`): the row's slot button, a button of the Safe Push form */
  trigger: ReactElement
  /** Open from outside, e.g. by the menu item of `getRequestReviewItem`; without it the popover keeps its own state */
  open?: boolean
  onOpenChange?: (open: boolean) => void
  align?: "start" | "center" | "end"
}

/**
 * Request Review… of an open pull request: the code owners Patronus suggests for the groups whose approval is missing
 * (one per group nobody is asked for yet chosen), a search of the repository's users, and "Request Review"; then a toast
 * with Open Pull Request and a refresh of the pull requests, so the readiness reads the new request
 * (`spec/branch/pull-requests.spec.md#request-review`). The form loads on the first hover or open.
 */
export function RequestReviewPopover({ branch, pullRequest, readiness, trigger, open, onOpenChange, align = "end" }: RequestReviewPopoverProps) {
  const [ownOpen, setOwnOpen] = useState(false)
  const setOpen = (nextOpen: boolean) => {
    if (open === undefined) {
      setOwnOpen(nextOpen)
    }
    onOpenChange?.(nextOpen)
  }
  return (
    <Popover open={open ?? ownOpen} onOpenChange={setOpen}>
      <PopoverTrigger render={trigger} onPointerEnter={() => void loadRequestReviewForm()} />
      <PopoverContent align={align} className="w-96 gap-3 p-3" data-testid="request-review-popover">
        <Suspense fallback={null}>
          <RequestReviewForm branch={branch} pullRequest={pullRequest} readiness={readiness} onDone={() => setOpen(false)} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}
