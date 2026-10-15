import type { ComponentType } from "react"
import { ChartPieIcon, CircleDashedIcon, GitMergeIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { describeMergedBranch, describeUnmergedBranch } from "@/lib/integration-commit"
import type { ArchivedBranch } from "@/stores/archived-branches"

/**
 * Integration status of an archived branch: "Merged" with the merge icon in `info` (the merged colour of the pull request,
 * Patronus and Landed chips), "Partially merged · 1 of 2" muted, "Not merged" `warning`, "Not checked" muted when the
 * detection failed. The counts and the merge commit are in the tooltip.
 */
export function ArchivedBranchStatusBadge({ branch }: { branch: ArchivedBranch }) {
  if (branch.type === "placeholder") {
    return (
      <div className="flex items-center gap-2">
        <Spinner className="size-3 text-muted-foreground" />
      </div>
    )
  }

  if (branch.type === "unknown") {
    return <ArchivedStatus status="unknown" icon={CircleDashedIcon} iconClassName="text-muted-foreground" text="Not checked" tooltip="Integration could not be checked, see the log" />
  }

  if (branch.type === "integrated") {
    return (
      <div className="flex items-center gap-2">
        <ArchivedStatus status="merged" icon={GitMergeIcon} iconClassName="text-info" text="Merged" tooltip={describeMergedBranch(branch.commitCount, branch.integrationCommit)} />
        {/* "High" is the usual confidence; only an exact match (the branch commits themselves) is named */}
        {branch.confidence === "Exact" && <Badge variant="success">Exact confidence</Badge>}
      </div>
    )
  }

  const tooltip = describeUnmergedBranch(branch.integratedCount, branch.commitCount)
  return branch.integratedCount > 0
    ? <ArchivedStatus status="partial" icon={ChartPieIcon} iconClassName="text-muted-foreground" text={`Partially merged · ${branch.integratedCount} of ${branch.commitCount}`} tooltip={tooltip} />
    : <ArchivedStatus status="not-merged" icon={TriangleAlertIcon} iconClassName="text-warning" text="Not merged" tooltip={tooltip} />
}

interface ArchivedStatusProps {
  status: "merged" | "partial" | "not-merged" | "unknown"
  icon: ComponentType<{ className?: string }>
  iconClassName: string
  text: string
  tooltip: string | null
}

function ArchivedStatus({ status, icon: Icon, iconClassName, text, tooltip }: ArchivedStatusProps) {
  return (
    <div className="flex items-center gap-2" data-testid="archived-branch-status" data-status={status}>
      <Icon className={cn("size-4 shrink-0", iconClassName)} />
      {tooltip === null
        ? <span className="text-sm text-muted-foreground">{text}</span>
        : (
            <Tooltip>
              <TooltipTrigger render={<span className="text-sm text-muted-foreground" />}>{text}</TooltipTrigger>
              <TooltipContent data-testid="archived-branch-status-tooltip">{tooltip}</TooltipContent>
            </Tooltip>
          )}
    </div>
  )
}
