import { TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { getBranchStackFixItems } from "@/hooks/useBranchContextActions"
import { useClosePreview } from "@/hooks/useClosePreview"
import type { StackProblemView } from "@/lib/bindings"
import { toPreviewMenu } from "@/lib/menu"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync } from "@/stores/branch-sync"
import { useOperations } from "@/stores/operations"

/** The preview handles no key of a fix */
const NO_KEYS: ReadonlySet<string> = new Set()

/**
 * Why a stacked branch can't be placed, as Rust says it (`StackProblemView`: the message, without `bd`), with its fixes
 * as the items of the branch menu that run them ("Stack On ci-traces", "Unstack", "Rename…"), each with its spinner
 * here while it runs; a fix that rewords a commit is the commit's "Reword…", marked in the list below. Inside the
 * branch preview a fix closes the preview first, as the preview's menu does.
 */
export function StackProblemAlert({ branch, problem }: { branch: Branch; problem: StackProblemView }) {
  const operations = useOperations()
  const isSyncing = useBranchSync(state => state.isSyncing)
  const closePreview = useClosePreview()
  const fixes = getBranchStackFixItems(branch, operations, isSyncing)
  const items = closePreview === null ? fixes : (toPreviewMenu([fixes], closePreview, NO_KEYS)[0] ?? [])
  return (
    <Alert variant="destructive" data-testid="branch-stack-problem">
      <TriangleAlertIcon />
      <AlertDescription className="flex flex-wrap items-center gap-x-3">
        <span>{`${problem.message}.`}</span>
        {items.map(item => (
          <MenuItemButton
            key={item.label}
            item={item}
            variant="link"
            surface={`stack-fix:${branch.name}:${item.label}`}
            className="h-auto px-0 text-destructive"
            data-testid="stack-problem-fix"
          />
        ))}
      </AlertDescription>
    </Alert>
  )
}
