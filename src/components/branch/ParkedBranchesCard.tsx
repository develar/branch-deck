import type { ReactNode } from "react"
import { cn } from "cn"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { HelpPopover } from "@/components/shared/HelpPopover"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { useParkDropTarget } from "@/hooks/useBranchDropTarget"
import { dropTargetState } from "@/hooks/useCommitDrag"
import { useSelectedProjectPath } from "@/stores/repository"
import { setParkedCollapsed, useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * The Parked card, after the branch table and its islands: the branches the user keeps local
 * ([Parked Branches](../../../spec/branch/parked-branches.spec.md)), their rows as in the table, open by default and
 * collapsed per repository. None of them is a move of the user: the header, the chips and F2 leave them out. The whole
 * card, also collapsed, parks the branches dragged onto it (`useParkDropTarget`); its rows leave such a drag to it.
 */
export function ParkedBranchesCard({ count, children }: { count: number; children: ReactNode }) {
  const repositoryPath = useSelectedProjectPath()
  const collapsed = useRecentProjects(projects => projects[0]?.parkedCollapsed === true)
  const dropTarget = useParkDropTarget()
  return (
    <CollapsibleCard
      data-testid="parked-branches-card"
      className={cn(dropTarget.canDrop && "ring-1 ring-primary/40", dropTarget.canDrop && dropTarget.isOver && "bg-primary/5 ring-2 ring-primary")}
      dropProps={dropTarget.dropProps}
      dropTarget={dropTargetState(dropTarget)}
      open={!collapsed}
      onOpenChange={open => {
        if (repositoryPath !== undefined) {
          setParkedCollapsed(repositoryPath, !open)
        }
      }}
      header={<SectionCardHeader title="Parked" count={count} itemSingular="branch" itemPlural="branches" />}
      help={<ParkedBranchesHelpPopover />}
    >
      {children}
    </CollapsibleCard>
  )
}

function ParkedBranchesHelpPopover() {
  return (
    <HelpPopover title="Parked" data-testid="parked-branches-help-icon">
      <p>A parked branch stays on your branch and in its build, but no push, Safe Push or Dry Run sends it, and it is no move of yours.</p>
      <p>A branch built on a parked one carries its commits: it is parked with it. Unpark a branch to land it.</p>
    </HelpPopover>
  )
}
