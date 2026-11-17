import { TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { getBranchStackFixItems } from "@/hooks/useBranchContextActions"
import { useClosePreview } from "@/hooks/useClosePreview"
import type { ActionId, StackProblemView } from "@/lib/bindings"
import { toPreviewMenu } from "@/lib/menu"
import { sameJson } from "@/lib/record"
import { useRowPart } from "@/model/store"
import type { Branch } from "@/stores/branch-sync"

/** The preview handles no key of a fix */
const NO_KEYS: ReadonlySet<string> = new Set()

/** The actions the fixes of a stack problem run (`getBranchStackFixItems`) */
const FIX_ACTIONS: readonly ActionId[] = ["stack-on", "unstack", "rename"]

/**
 * Why a stacked branch can't be placed, as Rust says it (`StackProblemView`: the message, without `bd`), with its fixes
 * as the items of the branch menu that run them ("Stack On ci-traces", "Unstack", "Rename…"), each with its spinner
 * here while it runs; a fix that rewords a commit is the commit's "Reword…", marked in the list below. Inside the
 * branch preview a fix closes the preview first, as the preview's menu does.
 */
export function StackProblemAlert({ branch, problem }: { branch: Branch; problem: StackProblemView }) {
  const closePreview = useClosePreview()
  // the actions of the fixes alone, kept while they say the same: the others change while the Safe Push of the branch
  // waits for the landing needs of a sync
  const actions = useRowPart(branch.name, "actions", all => all?.filter(action => FIX_ACTIONS.includes(action.id)), sameJson)
  const fixes = getBranchStackFixItems(branch, actions)
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
