import { cn } from "cn"
import type { LucideIcon } from "lucide-react"
import { GitBranchIcon, GitCommitHorizontalIcon, GitPullRequestIcon, SplitIcon, TriangleAlertIcon } from "lucide-react"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import type { ConflictLineData, ConflictMarkerType } from "@/lib/diff/conflict-annotations"

interface MarkerStyle {
  icon: LucideIcon
  iconClassName: string
  textClassName: string
  borderClassName: string
  description: string
}

const markerStyles: Record<ConflictMarkerType, MarkerStyle> = {
  // the side of the target: what is already in the branch the commit is copied onto
  "conflict-start": {
    icon: GitBranchIcon,
    iconClassName: "text-info",
    textClassName: "text-info",
    borderClassName: "border-info",
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
    icon: GitPullRequestIcon,
    iconClassName: "text-success",
    textClassName: "text-success",
    borderClassName: "border-success",
    description: "Incoming changes",
  },
}

const fallbackStyle: MarkerStyle = {
  icon: TriangleAlertIcon,
  iconClassName: "text-muted-foreground",
  textClassName: "",
  borderClassName: "border-border",
  description: "Conflict marker",
}

/** Git commit hashes are 40 hex characters (full) or commonly abbreviated to 7-8 */
function isCommitHash(value: string): boolean {
  return /^[0-9a-f]{7,40}$/i.test(value)
}

/** Annotation under a diff line with a conflict marker (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`), the `renderAnnotation` of `FileDiffView` */
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
      <div className="flex items-center gap-2 px-4 py-1.5 text-xs">
        <div className="flex items-center gap-2">
          <Icon className={cn("size-3.5", style.iconClassName)} />
          <span className={cn("font-medium", style.textClassName)}>{style.description}</span>
          <div className="text-muted-foreground">
            {reference &&
              (isCommitHash(reference) ? (
                <CommitHashPopover hash={reference} message={commit?.message} author={commit?.author} authorTime={commit?.authorTime} />
              ) : (
                <span>{reference}</span>
              ))}
          </div>
        </div>
      </div>
    </div>
  )
}
