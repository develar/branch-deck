import type { ReactNode } from "react"
import { ChevronDownIcon, ChevronUpIcon, EllipsisIcon, GitPullRequestIcon, XIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { DialogClose, DialogTitle } from "@/components/ui/dialog"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { WaitingAge } from "@/components/repository/ReviewRequestRow"
import { CopyButton } from "@/components/shared/CopyButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import type { ReviewRequest } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import type { ReviewRequestActions } from "@/lib/review-request-actions"
import type { SubmittedReview } from "@/stores/review-requests"

/** Where the review is shown: a dialog over the main window, or its own window */
export type ReviewChrome = "dialog" | "window"

export interface ReviewToolbarProps {
  chrome: ReviewChrome
  request: ReviewRequest
  /** The place of the request in the list, -1 once it left it */
  index: number
  count: number
  onPrevious: (() => void) | null
  onNext: (() => void) | null
  actions: ReviewRequestActions
}

/**
 * The row above the pull request: ‹ › and "2 of 5 · Your Turn" on the left; Open in Window, Open on GitHub, "⋯" (the
 * copy actions) and the close on the right. The window has no Open in Window and no close: it is the window.
 */
export function ReviewToolbar({ chrome, request, index, count, onPrevious, onNext, actions }: ReviewToolbarProps) {
  return (
    <div className="flex h-10 shrink-0 items-center gap-2 border-b border-border pr-2 pl-3 text-xs text-muted-foreground" data-testid="review-toolbar">
      <NavigationButton label="Previous request" keyHint="↑" icon={<ChevronUpIcon />} onClick={onPrevious} testId="review-previous" />
      <NavigationButton label="Next request" keyHint="↓" icon={<ChevronDownIcon />} onClick={onNext} testId="review-next" />
      <span className="truncate tabular-nums">
        {index === -1 ? "Your Turn" : <span data-testid="review-dialog-position">{`${index + 1} of ${count}`}</span>}
        {index !== -1 && " · Your Turn"}
      </span>
      <div className="ml-auto flex items-center gap-1">
        {actions.openInWindow !== null && <ToolbarButton item={actions.openInWindow} testId="review-open-in-window" />}
        <ToolbarButton item={actions.openPullRequest} label="GitHub" testId="review-open-pull-request" />
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger
              render={<DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" aria-label={`More actions for #${request.number}`} data-testid="review-dialog-more" />} />}
            >
              <EllipsisIcon />
            </TooltipTrigger>
            <TooltipContent>More actions</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end" className="w-auto min-w-36">
            <DropdownMenuItems groups={[[actions.copyLink, actions.copyBranchName]]} />
          </DropdownMenuContent>
        </DropdownMenu>
        {chrome === "dialog" && (
          <Tooltip>
            <TooltipTrigger render={<DialogClose render={<Button variant="ghost" size="icon-sm" aria-label="Close" data-testid="review-close" />} />}>
              <XIcon />
            </TooltipTrigger>
            <TooltipContent className="flex items-center gap-2">
              Close
              <Kbd>Esc</Kbd>
            </TooltipContent>
          </Tooltip>
        )}
      </div>
    </div>
  )
}

function NavigationButton({ label, keyHint, icon, onClick, testId }: { label: string, keyHint: string, icon: ReactNode, onClick: (() => void) | null, testId: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button variant="ghost" size="icon-sm" aria-label={label} disabled={onClick === null} focusableWhenDisabled data-testid={testId} onClick={() => onClick?.()} />
        )}
      >
        {icon}
      </TooltipTrigger>
      <TooltipContent className="flex items-center gap-2">
        {label}
        <Kbd>{keyHint}</Kbd>
      </TooltipContent>
    </Tooltip>
  )
}

/** A window-level action of the toolbar: the icon, a short label and the key */
function ToolbarButton({ item, label, testId }: { item: MenuItem, label?: string, testId: string }) {
  const Icon = item.icon ?? GitPullRequestIcon
  const key = item.kbds?.at(-1)
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="sm" data-testid={testId} onClick={item.onSelect} />}>
        <Icon data-icon="inline-start" />
        {label ?? item.label}
        {key !== undefined && <Kbd>{key.toUpperCase()}</Kbd>}
      </TooltipTrigger>
      <TooltipContent>{item.label}</TooltipContent>
    </Tooltip>
  )
}

export interface ReviewHeaderProps {
  chrome: ReviewChrome
  request: ReviewRequest
  submitted: SubmittedReview | undefined
  /** The review of the changes shows: the title alone, the rest gives the diffs room */
  compact: boolean
  onOpenOnGitHub: () => void
  /** "#101 got new commits while it was open…", the notices under the chips */
  children?: ReactNode
}

/**
 * Who asks and for how long ("bob asks for your review · waiting 2 days"), the title, and the chips: the number, `base ←
 * head`, the size, the checks, the decision of GitHub, the review of the user
 */
export function ReviewHeader({ chrome, request, submitted, compact, onOpenOnGitHub, children }: ReviewHeaderProps) {
  const title = <span className="min-w-0 text-lg leading-7 font-semibold text-foreground select-text">{request.title}</span>
  return (
    <div className="flex min-w-0 flex-col gap-2">
      {!compact && <AskedBy request={request} />}
      {chrome === "dialog" ? <DialogTitle className="contents">{title}</DialogTitle> : <h1 className="contents">{title}</h1>}
      {!compact && <RequestChips request={request} submitted={submitted} onOpenOnGitHub={onOpenOnGitHub} />}
      {children}
    </div>
  )
}

/** "bob asks for your review of alice's pull request · waiting 2 days" */
function AskedBy({ request }: { request: ReviewRequest }) {
  const asker = request.requestedBy ?? request.author
  const ofAuthor = request.author !== null && asker !== request.author ? ` of ${request.author}'s pull request` : ""
  return (
    <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground" data-testid="review-asked-by">
      <Avatar login={asker} />
      <span className="truncate">
        <span className="font-medium text-foreground">{asker ?? "Someone"}</span>
        {` asks for your review${ofAuthor}`}
      </span>
      <span aria-hidden>·</span>
      <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} testId="review-dialog-age" />
    </div>
  )
}

/** The initials of a login, no image from the network */
function Avatar({ login }: { login: string | null }) {
  const initials = (login ?? "?").split(/[-_.\s]+/).filter(part => part !== "").slice(0, 2).map(part => part[0]!.toUpperCase()).join("")
  return (
    <span aria-hidden className="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[0.625rem] font-semibold text-primary">
      {initials === "" ? "?" : initials}
    </span>
  )
}

/** "#101 · master ← alice/feature · 2 files +12 −3 · ● checks failing · approved by you" as chips */
function RequestChips({ request, submitted, onOpenOnGitHub }: { request: ReviewRequest, submitted: SubmittedReview | undefined, onOpenOnGitHub: () => void }) {
  const checks = request.checksState
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5" data-testid="review-dialog-meta">
      <Badge variant="outline" render={<button type="button" onClick={onOpenOnGitHub} />} className="cursor-pointer tabular-nums" aria-label={`Open pull request #${request.number} on GitHub`}>
        {`#${request.number}`}
      </Badge>
      <Badge variant="outline" className="max-w-full min-w-0 gap-1 pr-0.5 font-mono">
        <span className="min-w-0 truncate">{`${request.baseRefName} ← ${request.headRefName}`}</span>
        <CopyButton text={() => request.headRefName} tooltip="Copy Branch Name" label={`Copy branch name ${request.headRefName}`} alwaysVisible className="-my-1 text-muted-foreground" />
      </Badge>
      <Badge variant="outline" className="gap-1.5 tabular-nums">
        {request.changedFiles === 1 ? "1 file" : `${request.changedFiles} files`}
        <LineCounts additions={request.additions} deletions={request.deletions} colored />
      </Badge>
      {checks !== null && (
        <Badge variant="outline" className={cn("gap-1.5", checks === "failing" && "text-destructive", checks === "passing" && "text-success")} data-testid="review-dialog-checks">
          <StatusChipDot color={checks === "failing" ? "error" : checks === "pending" ? "warning" : "success"} testId="review-dialog-checks-dot" />
          {checks === "failing" ? "checks failing" : checks === "pending" ? "checks running" : "checks passed"}
        </Badge>
      )}
      {request.reviewDecision === "approved" && submitted === undefined && <Badge variant="success">approved</Badge>}
      {request.reviewDecision === "changesRequested" && submitted === undefined && <Badge variant="warning">changes requested</Badge>}
      {submitted !== undefined && (
        <Badge variant={submitted.event === "approve" ? "success" : "warning"} data-testid="review-dialog-submitted">
          {submitted.event === "approve" ? "approved by you" : "changes requested by you"}
        </Badge>
      )}
    </div>
  )
}
