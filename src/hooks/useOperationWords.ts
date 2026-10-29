import { useStore } from "zustand"
import type { ActionId, ActionState } from "@/lib/bindings"
import type { OperationNames, OperationWords } from "@/presenters/actions"
import { NO_OPERATIONS, useBranchSyncField } from "@/stores/branch-sync"
import { indicatorsStore } from "@/stores/indicators"

/** No busy state this window gave an operation: for a selector of a surface that names none */
const NO_NAMES: OperationNames = {}

/** Whether the item of `action` names an operation of the session: it runs as one, waits behind one, or one keeps it busy */
function namesOperation(action: ActionState): boolean {
  const { availability } = action
  return (
    availability.kind === "running" || availability.kind === "queued" || availability.kind === "waits" || (availability.kind === "disabled" && availability.reason.code === "busy")
  )
}

/** Whether an item `ids` of `actions` (all of them without `ids`) names an operation of the session (`useOperationWords`) */
export function namesOperations(actions: readonly ActionState[] | undefined, ids?: readonly ActionId[]): boolean {
  return actions?.some(action => (ids === undefined || ids.includes(action.id)) && namesOperation(action)) === true
}

/**
 * The operations of the session and the busy states this window gave its own (`BlockerWords`), selected only while
 * `names` (the surface names one), so a component renders for no other step of any operation of the repository
 */
export function useOperationWordsWhile(names: boolean): OperationWords {
  const operations = useBranchSyncField("operations", operations => (names ? operations : NO_OPERATIONS))
  const named = useStore(indicatorsStore, state => (names ? state.starts : NO_NAMES))
  return { operations, named }
}

/**
 * What the items `ids` of a subject's `actions` (as the caller selected them; all of them without `ids`) name of the
 * operations of the session (`useOperationWordsWhile`): selected only while one of those items runs, waits or is busy
 * with one
 */
export function useOperationWords(actions: readonly ActionState[] | undefined, ids?: readonly ActionId[]): OperationWords {
  return useOperationWordsWhile(namesOperations(actions, ids))
}
