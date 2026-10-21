import { memo, useEffect } from "react"
import type { ComponentType } from "react"
import { cn } from "cn"
import { BranchContextMenuRow } from "@/components/branch/BranchContextMenuRow"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { KeptBranchActionsCell, KeptBranchCommits, KeptBranchRowParts } from "@/components/branch/KeptBranchCommits"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { HelpPopover } from "@/components/shared/HelpPopover"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { TableHeader } from "@/components/shared/TableHeader"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useUnappliedBranchMenu } from "@/hooks/useUnappliedBranchMenu"
import { copyFocusedBranchRow } from "@/lib/clipboard"
import { formatCommitCount } from "@/lib/commit"
import type { MenuItemGroups } from "@/lib/menu"
import { getPushAction } from "@/lib/push-summary"
import { syncBranches } from "@/stores/branch-sync"
import { inlineSurface, useActiveInlineForRow, withRowProcessing } from "@/stores/inline-row-action"
import { useObjectBusy, useSurfaceBusy } from "@/stores/operations"
import type { UnappliedBranch } from "@/stores/unapplied-branches"
import { deleteUnappliedBranch, getUnappliedBranch, loadUnappliedCommits, useUnappliedBranch, useUnappliedBranchOrder } from "@/stores/unapplied-branches"

const columnCount = 3

/**
 * The Unapplied card, after the branch table and the Parked card, before the archived branches: the branches an unapply
 * kept ([Unapply Branch](../../../spec/branch/unapply.spec.md)). Their commits are not in HEAD, so they land nothing:
 * no robot, no move of the user, no row of the branch table. A row pushes its branch under its own name, so the remote
 * keeps a copy, reapplies it onto HEAD, and deletes it; open by default
 */
export function UnappliedBranchTableCard() {
  const order = useUnappliedBranchOrder()
  const { expandedKeys, toggleExpanded } = useRowExpansion()
  if (order.length === 0) {
    return null
  }
  return (
    <CollapsibleCard
      data-testid="unapplied-branches-card"
      defaultOpen
      header={<SectionCardHeader title="Unapplied" count={order.length} itemSingular="branch" itemPlural="branches" />}
      help={<UnappliedBranchesHelpPopover />}
    >
      <table className="w-full" onKeyDown={copyFocusedBranchRow}>
        <thead className="border-b border-border">
          <tr>
            <TableHeader>Branch Name</TableHeader>
            <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Remote</TableHeader>
            <TableHeader className="w-px px-4 whitespace-nowrap">Actions</TableHeader>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {order.map(name => (
            <UnappliedBranchRow key={name} name={name} expanded={expandedKeys.has(name)} onToggleExpanded={toggleExpanded} />
          ))}
        </tbody>
      </table>
    </CollapsibleCard>
  )
}

function UnappliedBranchesHelpPopover() {
  return (
    <HelpPopover title="Unapplied" data-testid="unapplied-branches-help-icon">
      <p>Unapply takes a branch and its files out of your branch; its commits wait here, and no sync archives or deletes them.</p>
      <p>An unapplied branch lands nothing: no Safe Push or Dry Run sends it. A push keeps a copy on the remote under its own name.</p>
      <p>Reapply puts its commits back on top of your branch, with their files and their branch prefix.</p>
    </HelpPopover>
  )
}

interface UnappliedBranchRowProps {
  name: string
  expanded: boolean
  onToggleExpanded: (name: string) => void
}

const UnappliedBranchRow = memo(function UnappliedBranchRow({ name, expanded, onToggleExpanded }: UnappliedBranchRowProps) {
  const branch = useUnappliedBranch(name)
  const activeInline = useActiveInlineForRow(name)
  // the delete runs from the form below the row, the push from a menu
  const processing = useSurfaceBusy(inlineSurface(name)) !== null
  const busy = useObjectBusy(name)
  const working = processing || busy !== null
  if (branch === undefined) {
    return null
  }
  const toggle = () => onToggleExpanded(name)
  return (
    <>
      <BranchContextMenuRow
        name={name}
        expanded={expanded}
        processing={working}
        className={cn(expanded && "bg-muted/50")}
        onToggle={toggle}
        menu={<UnappliedBranchMenuItems name={name} Items={ContextMenuItems} />}
      >
        <BranchNameCell name={branch.branch} summary={branch.subject} expanded={expanded} onToggleExpanded={toggle} />
        <td className="hidden w-px px-4 py-(--bd-cell-y) whitespace-nowrap text-muted-foreground sm:table-cell" data-testid="unapplied-branch-remote">
          {remoteText(branch)}
        </td>
        <KeptBranchActionsCell label={branch.branch} disabled={working} busy={busy} menu={<UnappliedBranchMenuItems name={name} Items={DropdownMenuItems} />} />
      </BranchContextMenuRow>

      <KeptBranchRowParts
        name={name}
        columnCount={columnCount}
        inline={activeInline}
        processing={processing}
        deleteForm="delete-unapplied"
        deleteTitle={`Delete the unapplied branch ${name}`}
        deleteHelpText="This deletes the unapplied branch and its commits; only a copy on the remote keeps them. This cannot be undone."
        onDelete={() => void confirmDelete(name)}
        expanded={expanded && <UnappliedBranchExpanded branch={branch} />}
      />
    </>
  )
})

/** "Not pushed", "Pushed", "2 commits to push", "Remote differs": what a push of it would do */
function remoteText(branch: UnappliedBranch): string {
  switch (getPushAction({ remoteStatus: branch.remoteStatus }) ?? "create") {
    case "create":
      return "Not pushed"
    case "upToDate":
      return "Pushed"
    case "push":
      return `${formatCommitCount(branch.remoteStatus?.commitsAhead ?? 0)} to push`
    case "force":
      return "Remote differs"
  }
}

/** Its commits, loaded on the first expansion */
function UnappliedBranchExpanded({ branch }: { branch: UnappliedBranch }) {
  const { name } = branch
  useEffect(() => {
    void loadUnappliedCommits(name)
  }, [name])
  return (
    <div className="ml-8 flex animate-in flex-col gap-2 border-l border-border pr-6 duration-150 fade-in-0">
      <KeptBranchCommits isLoading={branch.isLoadingCommits} loadError={branch.loadError} commits={branch.commits} />
    </div>
  )
}

/** The menu of an unapplied row, the same under the right-click (`ContextMenuItems`) and under "⋯" (`DropdownMenuItems`) */
function UnappliedBranchMenuItems({ name, Items }: { name: string; Items: ComponentType<{ groups: MenuItemGroups }> }) {
  return <Items groups={useUnappliedBranchMenu(name)} />
}

async function confirmDelete(name: string): Promise<void> {
  const branch = getUnappliedBranch(name)?.branch ?? name
  const deleted = await withRowProcessing(
    name,
    { id: "delete-unapplied", kind: "remote", label: `Deleting ${branch}` },
    async () => {
      await deleteUnappliedBranch(name)
      return true
    },
    {
      success: () => ({ title: `Deleted the unapplied branch ${branch}` }),
      errorTitle: () => `Cannot delete the unapplied branch ${branch}`,
      // the typed name stays
      keepFormOnError: true,
    },
  )
  if (deleted) {
    void syncBranches({ autoScroll: false, autoExpand: false })
  }
}
