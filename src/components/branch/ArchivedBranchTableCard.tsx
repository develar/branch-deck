import { lazy, memo, Suspense } from "react"
import { ChevronDownIcon, Trash2Icon } from "lucide-react"
import { cn } from "cn"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { ArchivedBranchExpanded } from "@/components/branch/ArchivedBranchExpanded"
import { ArchivedBranchesHelpPopover } from "@/components/branch/ArchivedBranchesHelpPopover"
import { ArchivedBranchStatusBadge } from "@/components/branch/ArchivedBranchStatusBadge"
import { ArchivedDateCell } from "@/components/branch/ArchivedDateCell"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { CopyButton } from "@/components/shared/CopyButton"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { TableHeader } from "@/components/shared/TableHeader"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { getSimpleBranchName } from "@/lib/branch"
import type { MenuItemGroups } from "@/lib/menu"
import { deleteArchivedBranch, useArchivedBranch, useArchivedBranchOrder } from "@/stores/archived-branches"
import { syncBranches } from "@/stores/branch-sync"
import { closeInline, isProcessing, openInline, portalTargetIdFor, useActiveInlineForRow, withRowProcessing } from "@/stores/inline-row-action"

const InlineDeleteConfirmationInput = lazy(() => import("@/components/branch/InlineDeleteConfirmationInput").then(it => ({ default: it.InlineDeleteConfirmationInput })))

const columnCount = 4

/** Collapsible table of archived branches (collapsed by default). */
export function ArchivedBranchTableCard() {
  const order = useArchivedBranchOrder()
  const { expandedKeys, toggleExpanded } = useRowExpansion()

  if (order.length === 0) {
    return null
  }

  return (
    <CollapsibleCard
      data-testid="archived-branches-card"
      header={(
        <SectionCardHeader
          title="Archived Branches"
          count={order.length}
          itemSingular="branch"
          itemPlural="branches"
          badgeColor="neutral"
          actions={(
            <>
              <ArchivedBranchesHelpPopover />
              <ChevronDownIcon className="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-180" />
            </>
          )}
        />
      )}
    >
      <table className="w-full">
        <thead className="border-b border-border bg-muted/50">
          <tr>
            <TableHeader>Branch Name</TableHeader>
            <TableHeader className="hidden sm:table-cell">Status</TableHeader>
            <TableHeader className="hidden sm:table-cell">Integrated</TableHeader>
            <TableHeader>Actions</TableHeader>
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

interface ArchivedBranchRowProps {
  name: string
  expanded: boolean
  onToggleExpanded: (name: string) => void
}

const ArchivedBranchRow = memo(function ArchivedBranchRow({ name, expanded, onToggleExpanded }: ArchivedBranchRowProps) {
  const branch = useArchivedBranch(name)
  const activeInline = useActiveInlineForRow(name)
  if (branch === undefined) {
    return null
  }

  const processing = activeInline?.processing === true
  const toggle = () => onToggleExpanded(name)

  return (
    <>
      <ContextMenu disabled={processing}>
        <ContextMenuTrigger
          render={(
            <tr
              data-branch-name={name}
              data-state={expanded ? "open" : "closed"}
              className={cn(
                "cursor-pointer transition-all hover:bg-muted",
                expanded && "bg-muted",
                branch.isLoadingCommits && "animate-pulse",
              )}
              onClick={toggle}
            />
          )}
        >
          <BranchNameCell
            name={name}
            summary={branch.summary}
            expanded={expanded}
            canExpand={branch.type !== "placeholder"}
            simplified
            onToggleExpanded={toggle}
          />
          <td className="hidden px-6 py-4 sm:table-cell">
            <ArchivedBranchStatusBadge branch={branch} />
          </td>
          <td className="hidden px-6 py-4 sm:table-cell">
            <ArchivedDateCell branch={branch} />
          </td>
          <td className="px-6 py-4">
            <CopyButton text={() => name} tooltip="Copy full branch name" alwaysVisible />
          </td>
        </ContextMenuTrigger>
        {/* the inline form opened from the menu takes the focus */}
        <ContextMenuContent finalFocus={false}>
          <ContextMenuItems groups={getArchivedContextMenuItems(name)} />
        </ContextMenuContent>
      </ContextMenu>

      {activeInline !== null && (
        <tr>
          <td id={portalTargetIdFor(name)} colSpan={columnCount} className="relative overflow-hidden p-0">
            {processing && <InlineRowProgressBar />}
            {activeInline.type === "delete-archived" && (
              <Suspense fallback={null}>
                <InlineDeleteConfirmationInput
                  key={`delete-archived:${name}`}
                  branchName={name}
                  title={`Delete archived branch ${name}`}
                  description="Type the branch name to confirm deletion"
                  onSubmit={() => void confirmDelete(name)}
                  onCancel={closeInline}
                />
              </Suspense>
            )}
          </td>
        </tr>
      )}

      {expanded && (
        <tr>
          <td colSpan={columnCount} className="px-0 py-4">
            <ArchivedBranchExpanded branch={branch} />
          </td>
        </tr>
      )}
    </>
  )
})

function getArchivedContextMenuItems(name: string): MenuItemGroups {
  return [
    getBranchCopyMenuItems(name, true),
    [{ label: "Delete Archived Branch", icon: Trash2Icon, onSelect: () => openInline("delete-archived", name) }],
  ]
}

async function confirmDelete(branchName: string): Promise<void> {
  if (isProcessing(branchName)) {
    return
  }

  const deleted = await withRowProcessing(
    branchName,
    async () => {
      await deleteArchivedBranch(branchName)
      return true
    },
    {
      success: () => ({ title: "Archived branch deleted", description: `${getSimpleBranchName(branchName)} was deleted` }),
      error: error => ({ title: "Failed to delete archived branch", description: error instanceof Error ? error.message : String(error) }),
    },
  )

  if (deleted) {
    void syncBranches({ autoScroll: false, autoExpand: false })
  }
}
