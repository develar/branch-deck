import { memo } from "react"
import type { ComponentType } from "react"
import { cn } from "cn"
import { Trash2Icon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ArchivedBranchesHelpPopover } from "@/components/branch/ArchivedBranchesHelpPopover"
import { ArchivedBranchExpanded } from "@/components/branch/ArchivedBranchExpanded"
import { ArchivedBranchStatusBadge } from "@/components/branch/ArchivedBranchStatusBadge"
import { ArchivedDateCell } from "@/components/branch/ArchivedDateCell"
import { BranchContextMenuRow } from "@/components/branch/BranchContextMenuRow"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { KeptBranchActionsCell, KeptBranchRowParts } from "@/components/branch/KeptBranchCommits"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { TableHeader } from "@/components/shared/TableHeader"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { getSimpleBranchName } from "@/lib/branch"
import { copyFocusedBranchRow } from "@/lib/clipboard"
import type { MenuItemGroups } from "@/lib/menu"
import { deleteArchivedBranch, useArchivedBranch, useArchivedBranchOrder, useIsNewlyArchived, useNotMergedArchivedCount } from "@/stores/archived-branches"
import { inlineSurface, openInline, useActiveInlineForRow, withRowProcessing } from "@/stores/inline-row-action"
import { useSurfaceBusy } from "@/stores/operations"

const columnCount = 4

/** Collapsible table of archived branches (collapsed by default). */
export function ArchivedBranchTableCard() {
  const order = useArchivedBranchOrder()
  const notMerged = useNotMergedArchivedCount()
  const { expandedKeys, toggleExpanded } = useRowExpansion()

  if (order.length === 0) {
    return null
  }

  return (
    <CollapsibleCard
      data-testid="archived-branches-card"
      header={
        <SectionCardHeader
          title="Archived Branches"
          count={order.length}
          itemSingular="branch"
          itemPlural="branches"
          badgeColor="neutral"
          note={notMerged > 0 && <NotMergedNote count={notMerged} />}
        />
      }
      help={<ArchivedBranchesHelpPopover />}
    >
      <table className="w-full" onKeyDown={copyFocusedBranchRow}>
        <thead className="border-b border-border">
          <tr>
            <TableHeader>Branch Name</TableHeader>
            <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Status</TableHeader>
            <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Integrated</TableHeader>
            <TableHeader className="w-px px-4 whitespace-nowrap">Actions</TableHeader>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {order.map(name => (
            <ArchivedBranchRow key={name} name={name} expanded={expandedKeys.has(name)} onToggleExpanded={toggleExpanded} />
          ))}
        </tbody>
      </table>
    </CollapsibleCard>
  )
}

/**
 * "3 not merged" in `warning` after the count of the collapsed card: the archived branches the baseline has none of the
 * commits of, whose work may be lost; their rows say "Not merged"
 */
function NotMergedNote({ count }: { count: number }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<span className="text-xs text-warning tabular-nums" data-testid="archived-not-merged" />}>{`${count} not merged`}</TooltipTrigger>
      <TooltipContent>{count === 1 ? "1 archived branch has commits the baseline has none of" : `${count} archived branches have commits the baseline has none of`}</TooltipContent>
    </Tooltip>
  )
}

interface ArchivedBranchRowProps {
  name: string
  expanded: boolean
  onToggleExpanded: (name: string) => void
}

const ArchivedBranchRow = memo(function ArchivedBranchRow({ name, expanded, onToggleExpanded }: ArchivedBranchRowProps) {
  const branch = useArchivedBranch(name)
  const isNew = useIsNewlyArchived(name)
  const activeInline = useActiveInlineForRow(name)
  // the delete runs from the form below the row
  const processing = useSurfaceBusy(inlineSurface(name)) !== null
  if (branch === undefined) {
    return null
  }

  const toggle = () => onToggleExpanded(name)

  return (
    <>
      <BranchContextMenuRow
        name={name}
        expanded={expanded}
        processing={processing}
        // "Loading commits…" below the row is the progress of its expansion
        className={cn(expanded && "bg-muted/50")}
        onToggle={toggle}
        menu={<ArchivedBranchMenuItems name={name} Items={ContextMenuItems} />}
      >
        <BranchNameCell
          name={name}
          summary={branch.summary}
          expanded={expanded}
          canExpand={branch.type !== "placeholder"}
          simplified
          mark={
            isNew && (
              <Badge variant="info" className="shrink-0" data-testid="archived-branch-new">
                New
              </Badge>
            )
          }
          onToggleExpanded={toggle}
        />
        <td className="hidden w-px px-4 py-(--bd-cell-y) whitespace-nowrap sm:table-cell">
          <ArchivedBranchStatusBadge branch={branch} />
        </td>
        <td className="hidden w-px px-4 py-(--bd-cell-y) whitespace-nowrap sm:table-cell">
          <ArchivedDateCell branch={branch} />
        </td>
        <KeptBranchActionsCell label={getSimpleBranchName(name)} disabled={processing} menu={<ArchivedBranchMenuItems name={name} Items={DropdownMenuItems} />} />
      </BranchContextMenuRow>

      <KeptBranchRowParts
        name={name}
        columnCount={columnCount}
        inline={activeInline}
        processing={processing}
        deleteForm="delete-archived"
        deleteTitle={`Delete the archived branch ${name}`}
        onDelete={() => void confirmDelete(name)}
        expanded={expanded && <ArchivedBranchExpanded branch={branch} />}
      />
    </>
  )
})

/** The menu of an archived row, the same under the right-click (`ContextMenuItems`) and under "⋯" (`DropdownMenuItems`) */
function ArchivedBranchMenuItems({ name, Items }: { name: string; Items: ComponentType<{ groups: MenuItemGroups }> }) {
  return <Items groups={getArchivedContextMenuItems(name)} />
}

function getArchivedContextMenuItems(name: string): MenuItemGroups {
  return [getBranchCopyMenuItems(name, true), [{ label: "Delete Archived Branch…", icon: Trash2Icon, color: "destructive", onSelect: () => openInline("delete-archived", name) }]]
}

/** Deletes the archived branch; the session's patch removes its row, no sync follows */
async function confirmDelete(branchName: string): Promise<void> {
  await withRowProcessing(branchName, { id: "delete-archived", kind: "remote", label: `Deleting ${getSimpleBranchName(branchName)}` }, () => deleteArchivedBranch(branchName), {
    success: () => ({ title: `Deleted the archived branch ${getSimpleBranchName(branchName)}` }),
    errorTitle: () => `Cannot delete the archived branch ${getSimpleBranchName(branchName)}`,
    // the typed name stays
    keepFormOnError: true,
  })
}
