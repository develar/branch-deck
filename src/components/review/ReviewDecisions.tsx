import { useState } from "react"
import type { RefObject } from "react"
import { CheckIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Popover, PopoverContent } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { MenuItemButton, PRIMARY_KBD_CLASS_NAME } from "@/components/shared/MenuItemButton"
import { KeyHint } from "@/components/shared/PreviewDialogFrame"
import type { ReviewRequest } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { isMac } from "@/lib/platform"
import { readReviewDraft, writeReviewDraft } from "@/lib/review-draft"
import type { ReviewRequestActions } from "@/lib/review-request-actions"
import type { ReviewAction } from "@/stores/review-requests"

export type ReviewTab = "overview" | "changes"

export interface ReviewTabsProps {
  tab: ReviewTab
  onTabChange: (tab: ReviewTab) => void
  /**
   * The files of the range once fetched; null while the pull request is fetched or its fetch failed: Changes waits for
   * it, the rail says how far the fetch is (its only progress) or the alert why it failed
   */
  fileCount: number | null
}

/** Overview and Changes (D): which view of the pull request shows, the way into the review of its diffs */
export function ReviewTabs({ tab, onTabChange, fileCount }: ReviewTabsProps) {
  return (
    <Tabs value={tab} onValueChange={value => onTabChange(value as ReviewTab)}>
      <TabsList variant="line" className="h-7" data-testid="review-tabs">
        <TabsTrigger value="overview" className="px-2" data-testid="review-tab-overview">
          Overview
        </TabsTrigger>
        <TabsTrigger value="changes" className="px-2" disabled={fileCount === null} data-testid="review-dialog-review">
          Changes
          {fileCount !== null && <span className="text-muted-foreground tabular-nums">{fileCount}</span>}
          <Kbd>D</Kbd>
        </TabsTrigger>
      </TabsList>
    </Tabs>
  )
}

export interface ReviewDecisionsProps {
  request: ReviewRequest
  actions: ReviewRequestActions
  busy: ReviewAction | undefined
  /** The commit a review is on; null while there is none */
  commitId: string | null
  requestChangesRef: RefObject<HTMLButtonElement | null>
  approveRef: RefObject<HTMLButtonElement | null>
  safePushRef: RefObject<HTMLButtonElement | null>
  isRequestingChanges: boolean
  onRequestingChangesChange: (open: boolean) => void
  isConfirmingSafePush: boolean
  onConfirmingSafePushChange: (open: boolean) => void
  onRequestChanges: (body: string) => Promise<boolean>
  onSafePush: () => Promise<boolean>
}

/**
 * The decisions on the pull request, the primary last: Request Changes… (R), Approve and Safe Push… (M) where Patronus
 * merges, and Approve (A), the primary. Once the user approved the commit, "Approved" says so and Safe Push…, the next
 * step, is the primary.
 */
export function ReviewDecisions(props: ReviewDecisionsProps) {
  const { request, actions, busy, commitId, requestChangesRef, approveRef, safePushRef } = props
  const safePushNext = actions.approved && actions.safePush !== null && actions.safePush.disabled !== true
  return (
    <div className="ml-auto flex flex-wrap items-center justify-end gap-2" data-testid="review-decisions">
      <MenuItemButton item={actions.requestChanges} ref={requestChangesRef} variant="outline" size="sm" pending={busy === "requestChanges"} data-testid="review-request-changes" />
      <RequestChangesPopover
        request={request}
        commitId={commitId}
        anchor={requestChangesRef}
        open={props.isRequestingChanges && commitId !== null}
        onOpenChange={props.onRequestingChangesChange}
        onSend={props.onRequestChanges}
      />
      {actions.safePush !== null && (
        <>
          <MenuItemButton
            item={actions.safePush}
            ref={safePushRef}
            variant={safePushNext ? "default" : "outline"}
            size="sm"
            pending={busy === "safePush"}
            data-testid="review-safe-push"
          />
          <SafePushPopover
            request={request}
            commitId={commitId}
            approved={actions.approved}
            pending={busy === "safePush" || busy === "approve"}
            anchor={safePushRef}
            open={props.isConfirmingSafePush && commitId !== null && actions.safePush.disabled !== true}
            onOpenChange={props.onConfirmingSafePushChange}
            onStart={props.onSafePush}
          />
        </>
      )}
      {actions.approved ? (
        <ApprovedBadge description={actions.approve.description} />
      ) : (
        <MenuItemButton item={actions.approve} ref={approveRef} variant="default" size="sm" pending={busy === "approve"} data-testid="review-approve" />
      )}
    </div>
  )
}

/** "✓ Approved": the approval of the shown commit is done; the hover names the commit */
function ApprovedBadge({ description }: { description: string | undefined }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Badge variant="success" tabIndex={0} className="h-7 px-2.5 text-sm" data-testid="review-approve" data-approved />}>
        <CheckIcon data-icon="inline-start" />
        Approved
      </TooltipTrigger>
      {description !== undefined && <TooltipContent>{description}</TooltipContent>}
    </Tooltip>
  )
}

/**
 * What Approve and Safe Push does, before it does it: the approval of the commit the user saw, then the `/patronus
 * merge` comment, which merges and deletes the branch; Enter confirms. While Patronus is asked, the form says so.
 */
function SafePushPopover({
  request,
  commitId,
  approved,
  pending,
  anchor,
  open,
  onOpenChange,
  onStart,
}: {
  request: ReviewRequest
  commitId: string | null
  approved: boolean
  pending: boolean
  anchor: RefObject<HTMLButtonElement | null>
  open: boolean
  onOpenChange: (open: boolean) => void
  onStart: () => Promise<boolean>
}) {
  const start = async () => {
    if (commitId !== null && !pending && (await onStart())) {
      onOpenChange(false)
    }
  }
  const hash = commitId === null ? "" : formatShortHash(commitId)
  return (
    <Popover open={open} onOpenChange={next => !pending && onOpenChange(next)}>
      {/* opened by a key or the row's menu as well, without a trigger: the focus goes back to the button */}
      <PopoverContent anchor={anchor} finalFocus={anchor} side="bottom" align="end" className="w-96" data-testid="review-safe-push-form">
        <p className="text-sm" data-testid="review-safe-push-description">
          {`${approved ? "Comments" : `Approves ${hash}, then comments`} /patronus merge on #${request.number}: Patronus rebases ${request.headRefName} onto ${request.baseRefName}, runs the checks, merges it and deletes the branch. It reports in the pull request.`}
        </p>
        <div className="flex items-center justify-end gap-3 text-xs text-muted-foreground">
          {pending ? (
            <span className="mr-auto flex items-center gap-2" data-testid="review-safe-push-progress">
              <Spinner className="size-3" />
              {`Asking Patronus on pull request #${request.number}…`}
            </span>
          ) : (
            <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
          )}
          <Button size="xs" autoFocus disabled={pending || commitId === null} data-testid="review-safe-push-start" onClick={() => void start()}>
            {approved ? "Safe Push" : "Approve and Safe Push"}
            <Kbd className={PRIMARY_KBD_CLASS_NAME}>↵</Kbd>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

/**
 * The comment of a request for changes, required: ⌘↵ sends it, Escape closes the form and keeps the draft, also when the
 * window shows another pull request or closes (`readReviewDraft`)
 */
function RequestChangesPopover({
  request,
  commitId,
  anchor,
  open,
  onOpenChange,
  onSend,
}: {
  request: ReviewRequest
  commitId: string | null
  anchor: RefObject<HTMLButtonElement | null>
  open: boolean
  onOpenChange: (open: boolean) => void
  onSend: (body: string) => Promise<boolean>
}) {
  const [draft, setDraft] = useState(() => readReviewDraft(request.url))
  const [draftOf, setDraftOf] = useState(request.url)
  if (draftOf !== request.url) {
    setDraftOf(request.url)
    setDraft(readReviewDraft(request.url))
  }
  const changeDraft = (next: string) => {
    setDraft(next)
    writeReviewDraft(request.url, next)
  }
  const [isSending, setSending] = useState(false)
  const canSend = draft.trim() !== "" && commitId !== null && !isSending

  const send = async () => {
    if (!canSend) {
      return
    }
    setSending(true)
    const sent = await onSend(draft)
    setSending(false)
    if (sent) {
      changeDraft("")
      onOpenChange(false)
    }
  }

  return (
    <Popover open={open} onOpenChange={next => onOpenChange(next)}>
      {/* opened by a key or the row's menu as well, without a trigger: the focus goes back to the button */}
      <PopoverContent anchor={anchor} finalFocus={anchor} side="bottom" align="end" className="w-96" data-testid="review-request-changes-form">
        <Textarea
          aria-label={`What #${request.number} should change`}
          data-testid="review-request-changes-comment"
          className="min-h-28"
          placeholder="What should change"
          value={draft}
          disabled={isSending}
          autoFocus
          onChange={event => changeDraft(event.target.value)}
          onKeyDown={event => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              void send()
            }
          }}
        />
        <div className="flex items-center justify-end gap-3 text-xs text-muted-foreground">
          <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
          <Button size="xs" disabled={!canSend} data-testid="review-request-changes-send" onClick={() => void send()}>
            {isSending && <Spinner data-icon="inline-start" />}
            Request Changes
            <Kbd className={PRIMARY_KBD_CLASS_NAME}>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}
