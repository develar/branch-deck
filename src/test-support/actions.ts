// The actions of the subjects as Rust offers them (`RepositoryView.actions`, `domain_policy::actions`), for the tests of
// the surfaces that present them: a test sets what Rust answers, never derives it
import type { ActionBlocker, ActionId, ActionState } from "@/lib/bindings"
import { branchSyncStore } from "@/stores/branch-sync"

/** Each of `ids` offered and enabled */
export function enabled(...ids: ActionId[]): ActionState[] {
  return ids.map(id => ({ id, availability: { kind: "enabled" } }))
}

/** `id` offered and disabled for `reason` */
export function disabled(id: ActionId, reason: ActionBlocker): ActionState {
  return { id, availability: { kind: "disabled", reason } }
}

/** The actions of a branch that lands, as Rust offers them on Space with nothing in the way: the starts, the push, the review, the rewrites, the park */
export const LANDING_BRANCH: readonly ActionId[] = [
  "safe-push",
  "safe-push-with-profile",
  "dry-run",
  "check-approvals",
  "push",
  "review-changes",
  "create-pull-request",
  "rename",
  "move-to",
  "stack-on",
  "ungroup",
  "amend-changes",
  "add-issue-reference",
  "squash-fixups",
  "unapply",
  "park",
]

/** `actions` with the state of each id of `changes` replaced, an id it lacks added */
export function withActions(actions: readonly ActionState[], ...changes: ActionState[]): ActionState[] {
  const changed = new Map(changes.map(change => [change.id, change]))
  const kept = actions.map(action => changed.get(action.id) ?? action)
  return [...kept, ...changes.filter(change => !actions.some(action => action.id === change.id))]
}

/** The actions of each subject (a branch, `stack:<root>`, `robot:<id>`) as the view holds them */
export function setActions(actions: Readonly<Record<string, readonly ActionState[]>>): void {
  branchSyncStore.setState({ actions })
}
