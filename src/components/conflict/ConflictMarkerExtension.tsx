import type { LucideIcon } from "lucide-react"
import { CircleAlertIcon, GitBranchIcon, GitCommitHorizontalIcon, GitMergeIcon, SplitIcon } from "lucide-react"
import { cn } from "cn"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import type { ConflictMarkerCommitInfo } from "@/lib/bindings"

export type ConflictMarkerType = "conflict-start" | "conflict-base" | "conflict-separator" | "conflict-end"

export interface ConflictMarkerInfo {
  type: ConflictMarkerType
  tooltip: string
  branch?: string
  base?: string
  commit?: string
}

/** `extendData` payload of a diff line holding a conflict marker */
export interface ConflictLineData {
  tooltip: string
  conflictInfo?: ConflictMarkerInfo
  conflictMarkerCommits?: Record<string, ConflictMarkerCommitInfo>
}

interface MarkerStyle {
  icon: LucideIcon
  iconClassName: string
  textClassName: string
  borderClassName: string
  description: string
}

const markerStyles: Record<ConflictMarkerType, MarkerStyle> = {
  "conflict-start": {
    icon: GitBranchIcon,
    iconClassName: "text-destructive",
    textClassName: "text-destructive",
    borderClassName: "border-destructive",
    description: "Current changes (HEAD)",
  },
  "conflict-base": {
    icon: GitCommitHorizontalIcon,
    iconClassName: "text-muted-foreground",
    textClassName: "text-muted-foreground",
    borderClassName: "border-border",
    description: "Common ancestor",
  },
  "conflict-separator": {
    icon: SplitIcon,
    iconClassName: "text-warning",
    textClassName: "text-warning",
    borderClassName: "border-warning",
    description: "Conflict separator",
  },
  "conflict-end": {
    icon: GitMergeIcon,
    iconClassName: "text-success",
    textClassName: "text-success",
    borderClassName: "border-success",
    description: "Incoming changes",
  },
}

const fallbackStyle: MarkerStyle = {
  icon: CircleAlertIcon,
  iconClassName: "text-muted-foreground",
  textClassName: "",
  borderClassName: "border-border",
  description: "Conflict marker",
}

/** Git commit hashes are 40 hex characters (full) or commonly abbreviated to 7-8 */
function isCommitHash(value: string): boolean {
  return /^[0-9a-f]{7,40}$/i.test(value)
}

/** Annotation under a diff line with a conflict marker (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`); rendered by `DiffView` `renderExtendLine`. */
export function ConflictMarkerExtension({ data }: { data: ConflictLineData }) {
  const conflictInfo = data.conflictInfo
  if (!conflictInfo?.type) {
    return null
  }

  const style = markerStyles[conflictInfo.type] ?? fallbackStyle
  const Icon = style.icon
  // the branch, base or commit named by the marker
  const reference = conflictInfo.branch || conflictInfo.base || conflictInfo.commit
  const commit = reference ? data.conflictMarkerCommits?.[reference] : undefined

  return (
    <div className={cn("border-l-4 bg-muted", style.borderClassName)}>
      <div className="flex items-center gap-2 px-4 py-2">
        <div className="flex items-center gap-2">
          <Icon className={cn("size-4", style.iconClassName)} />
          <span className={cn("text-sm font-medium", style.textClassName)}>{style.description}</span>
          <div className="text-xs text-muted-foreground">
            {reference && (isCommitHash(reference)
              ? <CommitHashPopover hash={reference} message={commit?.message} author={commit?.author} authorTime={commit?.authorTime} />
              : <span>{reference}</span>)}
          </div>
        </div>
      </div>
    </div>
  )
}
