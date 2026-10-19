import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronDownIcon, ChevronUpIcon, EllipsisIcon, GitPullRequestIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { WaitingAge } from "@/components/repository/ReviewRequestRow"
import { ActionButton } from "@/components/shared/ActionButton"
import { CopyButton } from "@/components/shared/CopyButton"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { WindowTitle } from "@/components/shared/WindowTitleBar"
import type { ReviewRequest } from "@/lib/bindings"
import { formatFileCount } from "@/lib/commit-files"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import type { ReviewRequestActions } from "@/lib/review-request-actions"
import type { SubmittedReview } from "@/stores/review-requests"

export interface ReviewToolbarProps {
  request: ReviewRequest
  /** The place of the request in the list, -1 once it left it */
  index: number
  count: number
  onPrevious: (() => void) | null
  onNext: (() => void) | null
  actions: ReviewRequestActions
  /** The changes show: Unified/Split switches their diffs */
  reviewing: boolean
}

/** ↑ ↓ and "Your Turn › 2 of 5": where the request is in the list, the start of the title bar of the review window */
export function ReviewNavigation({ index, count, onPrevious, onNext }: Pick<ReviewToolbarProps, "index" | "count" | "onPrevious" | "onNext">) {
  return (
    <div className="flex min-w-0 items-center gap-2" data-testid="review-navigation">
      <NavigationButton label="Previous request" keyHint="↑" icon={<ChevronUpIcon />} onClick={onPrevious} testId="review-previous" />
      <NavigationButton label="Next request" keyHint="↓" icon={<ChevronDownIcon />} onClick={onNext} testId="review-next" />
      <WindowTitle parts={index === -1 ? ["Your Turn"] : ["Your Turn", { text: `${index + 1} of ${count}`, testId: "review-dialog-position" }]} className="tabular-nums" />
    </div>
  )
}

/** The diff view while the changes show, Open on GitHub and "⋯" (the copy actions): the end of the title bar */
export function ReviewToolbarActions({ request, actions, reviewing }: Pick<ReviewToolbarProps, "request" | "actions" | "reviewing">) {
  return (
    <>
      {reviewing && <DiffViewToggle />}
      <MenuItemAction item={actions.openPullRequest} label="GitHub" data-testid="review-open-pull-request" />
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
    </>
  )
}

function NavigationButton({ label, keyHint, icon, onClick, testId }: { label: string; keyHint: string; icon: ReactNode; onClick: (() => void) | null; testId: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={<Button variant="ghost" size="icon-sm" aria-label={label} disabled={onClick === null} focusableWhenDisabled data-testid={testId} onClick={() => onClick?.()} />}
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

/** A window-level action of the toolbar (`ActionButton`): the icon, a short label and the key */
function MenuItemAction({ item, label, "data-testid": testId }: { item: MenuItem; label?: string; "data-testid": string }) {
  return (
    <ActionButton
      label={label ?? item.label}
      icon={item.icon ?? GitPullRequestIcon}
      keyHint={item.kbds?.at(-1)}
      disabled={item.disabled === true}
      description={item.disabled === true ? (item.description ?? item.label) : item.label}
      onClick={() => item.onSelect(FROM_MENU)}
      data-testid={testId}
    />
  )
}

export interface ReviewHeaderProps {
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
export function ReviewHeader({ request, submitted, compact, onOpenOnGitHub, children }: ReviewHeaderProps) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      {!compact && <AskedBy request={request} />}
      <h2 className="min-w-0 text-base leading-6 font-medium text-pretty text-foreground select-text">{request.title}</h2>
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
  const initials = (login ?? "?")
    .split(/[-_.\s]+/)
    .filter(part => part !== "")
    .slice(0, 2)
    .map(part => part[0]!.toUpperCase())
    .join("")
  return (
    <span aria-hidden className="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary/10 bd-avatar-initials font-semibold text-primary">
      {initials === "" ? "?" : initials}
    </span>
  )
}

/** "#101 · master ← alice/feature · 2 files +12 −3 · ● checks failing · approved by you" as chips */
function RequestChips({ request, submitted, onOpenOnGitHub }: { request: ReviewRequest; submitted: SubmittedReview | undefined; onOpenOnGitHub: () => void }) {
  const checks = request.checksState
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5" data-testid="review-dialog-meta">
      <Badge
        variant="outline"
        render={<button type="button" onClick={onOpenOnGitHub} />}
        className="cursor-pointer tabular-nums"
        aria-label={`Open pull request #${request.number} on GitHub`}
      >
        {`#${request.number}`}
      </Badge>
      <Badge variant="outline" className="max-w-full min-w-0 gap-1 pr-0.5 font-mono">
        <span className="min-w-0 truncate">{`${request.baseRefName} ← ${request.headRefName}`}</span>
        <CopyButton
          text={() => request.headRefName}
          tooltip="Copy Branch Name"
          label={`Copy branch name ${request.headRefName}`}
          alwaysVisible
          className="-my-1 text-muted-foreground"
        />
      </Badge>
      <Badge variant="outline" className="gap-1.5 tabular-nums">
        {formatFileCount(request.changedFiles)}
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
