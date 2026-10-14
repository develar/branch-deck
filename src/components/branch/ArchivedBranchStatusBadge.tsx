import { ChartPieIcon, GitMergeIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { IntegrationConfidence } from "@/lib/bindings"
import { describeIntegrationCommit } from "@/lib/integration-commit"
import type { ArchivedBranch } from "@/stores/archived-branches"

function confidenceBadgeVariant(confidence: IntegrationConfidence): "success" | "info" {
  return confidence === "Exact" ? "success" : "info"
}

/** Integration status of an archived branch ("Merged", "Partially merged", "Not merged"). */
export function ArchivedBranchStatusBadge({ branch }: { branch: ArchivedBranch }) {
  if (branch.type === "placeholder") {
    return (
      <div className="flex items-center gap-2">
        <Spinner className="size-3 text-muted-foreground" />
      </div>
    )
  }

  if (branch.type === "integrated") {
    // only a non-default confidence is shown
    const showConfidence = branch.confidence !== undefined && branch.confidence !== "High"
    const label = `Merged · ${branch.commitCount}`
    return (
      <div className="flex items-center gap-2">
        <GitMergeIcon className="size-4 text-info" />
        <div className="flex items-center gap-2">
          {branch.integrationCommit === undefined
            ? <span className="text-sm text-muted-foreground">{label}</span>
            : (
                <Tooltip>
                  <TooltipTrigger render={<span className="text-sm text-muted-foreground" />}>{label}</TooltipTrigger>
                  <TooltipContent>{describeIntegrationCommit(branch.integrationCommit)}</TooltipContent>
                </Tooltip>
              )}
          {showConfidence && (
            <Badge variant={confidenceBadgeVariant(branch.confidence!)}>
              {branch.confidence}
              {" confidence"}
            </Badge>
          )}
        </div>
      </div>
    )
  }

  const partial = branch.integratedCount > 0
  const Icon = partial ? ChartPieIcon : TriangleAlertIcon
  return (
    <div className="flex items-center gap-2">
      <Icon className={cn("size-4", partial ? "text-info" : "text-warning")} />
      <Tooltip>
        <TooltipTrigger render={<span className="text-sm text-muted-foreground" />}>
          {partial ? `Partially merged · ${branch.integratedCount}/${branch.commitCount}` : `Not merged · ${branch.commitCount}`}
        </TooltipTrigger>
        <TooltipContent>
          {partial
            ? `Partially merged: ${branch.integratedCount} of ${branch.commitCount} commits merged`
            : "Original commits are missing from HEAD"}
        </TooltipContent>
      </Tooltip>
    </div>
  )
}
