import { ChartPieIcon, CircleCheckIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { IntegrationConfidence } from "@/lib/bindings"
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
    return (
      <div className="flex items-center gap-2">
        <CircleCheckIcon className="size-4 text-success" />
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{`Merged · ${branch.commitCount}`}</span>
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
