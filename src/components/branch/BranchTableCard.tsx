import { Card } from "@/components/ui/card"
import { BranchRow } from "@/components/branch/BranchRow"
import { TableHeader } from "@/components/shared/TableHeader"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useTauriListen } from "@/hooks/useTauriListen"
import { syncBranches, useBranchOrder } from "@/stores/branch-sync"

/** Table of active virtual branches. Subscribes to the branch order only; each row subscribes to its own branch. */
export function BranchTableCard() {
  const branchOrder = useBranchOrder()
  const { expandedKeys, toggleExpanded, setExpanded } = useRowExpansion()

  // "Sync" menu item
  useTauriListen("sync-branches", () => {
    void syncBranches()
  })

  if (branchOrder.length === 0) {
    return null
  }

  return (
    <Card className="gap-0 py-0">
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead className="border-b border-border bg-muted/50">
            <tr>
              <TableHeader>Branch Name</TableHeader>
              <TableHeader className="hidden sm:table-cell">Commits</TableHeader>
              <TableHeader className="hidden sm:table-cell">Status</TableHeader>
              <TableHeader>Actions</TableHeader>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {branchOrder.map(name => (
              <BranchRow
                key={name}
                name={name}
                expanded={expandedKeys.has(name)}
                onToggleExpanded={toggleExpanded}
                setExpanded={setExpanded}
              />
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  )
}
