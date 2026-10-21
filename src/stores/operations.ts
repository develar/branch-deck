import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { MenuItem } from "@/lib/menu"
import type { RunningMove } from "@/lib/running-move"
import { getSelectedProject } from "@/stores/repository"

/**
 * What an action does to the repository: a rewrite moves HEAD or a ref of Branch Deck under the repository lock (a
 * regroup, a squash, an amend), so two of them never run at once; a remote action talks to a server (a pull request, a
 * robot) or deletes a ref of its own
 */
export type OperationKind = "rewrite" | "remote"

/** What a running action is checked by: the same action of the same object joins it, a conflicting one waits */
export interface OperationKey {
  /** The action, as the `id` of its menu item ("unstack", "move-to") */
  readonly id: string
  /** The object it acts on (`branchSubject`): its row shows the spinner of a start from a menu or a key */
  readonly subject: string
  /** The branches it works on: their rows are busy while it runs */
  readonly objects: readonly string[]
  readonly kind: OperationKind
  /** The repository; the selected one by default */
  readonly repositoryPath?: string
}

export interface OperationSpec extends OperationKey {
  /** The busy state in sentence case, without the ellipsis: "Unstacking tool-events" */
  readonly label: string
  /**
   * The surface that started it (`useIndicatorClaim`): a button that stays shows the spinner itself; null for a menu, a
   * key or a toast, whose row (the trigger of the object's menu) shows it
   */
  readonly surface?: string | null
  /**
   * The action is the move of a row (a review request, a restart of its robot): its node shows the move made while it
   * runs (`getOperationMoves`)
   */
  readonly move?: RunningMove
}

export interface Operation extends OperationSpec {
  readonly key: number
  readonly repositoryPath: string
  readonly surface: string | null
}

export type Operations = readonly Operation[]

interface OperationsState {
  /** The actions that run, in their start order */
  running: Operations
  /** The surfaces on the page that show the spinner of what they started, by the count of their mounts */
  claims: Readonly<Record<string, number>>
}

const NO_OPERATIONS: Operations = []

export const operationsStore = createStore<OperationsState>(() => ({ running: NO_OPERATIONS, claims: {} }))

/** The running promise of each operation: a second start of the same action resolves with it */
const promises = new Map<number, Promise<unknown>>()
let nextKey = 1

/** The subject of a branch: the row whose menu and keys start its actions */
export function branchSubject(name: string): string {
  return `branch:${name}`
}

function repositoryOf(key: OperationKey): string {
  return key.repositoryPath ?? getSelectedProject()?.path ?? ""
}

/** The running operation that `key` joins: the same action of the same object */
export function findSame(running: Operations, key: OperationKey): Operation | undefined {
  const repositoryPath = repositoryOf(key)
  return running.find(it => it.id === key.id && it.subject === key.subject && it.repositoryPath === repositoryPath)
}

/**
 * The running operation that `key` must wait for, null when it can run: a rewrite waits for any rewrite of the
 * repository (all of them move HEAD or a ref of Branch Deck under one lock), a remote action for a remote action of
 * the same object or of a branch it works on. A rewrite and a remote action run side by side: a push waits for the
 * sync after the rewrite itself.
 */
export function getOperationBlocker(running: Operations, key: OperationKey): Operation | null {
  const repositoryPath = repositoryOf(key)
  for (const operation of running) {
    if (operation.repositoryPath !== repositoryPath || operation.kind !== key.kind) {
      continue
    }
    if (key.kind === "rewrite" || operation.subject === key.subject || operation.objects.some(name => key.objects.includes(name))) {
      return operation
    }
  }
  return null
}

/** The reason an action can't start while `operation` runs: its busy state, "Unstacking tool-events…" */
export function formatOperationBlocker(operation: Operation): string {
  return `${operation.label}…`
}

/**
 * The busy state of a rewrite of the selected repository that runs ("Parking docs…"), null when none runs: a drop waits
 * for it, since what was dragged may have other ids once it ends
 */
export function getRunningRewrite(): string | null {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const rewrite = operationsStore.getState().running.find(operation => operation.kind === "rewrite" && operation.repositoryPath === repositoryPath)
  return rewrite === undefined ? null : formatOperationBlocker(rewrite)
}

/** The running operations; a component reads them with `useOperations` */
export function getOperations(): Operations {
  return operationsStore.getState().running
}

/**
 * The item of an action of an object as the running operations leave it: running while it runs itself (a second start
 * joins it), disabled with the busy state of the operation it waits for, else as it is. An item already disabled keeps
 * its reason; a submenu (Stack On ›) is disabled as a whole.
 */
export function operationItem(running: Operations, item: MenuItem, key: OperationKey): MenuItem {
  const same = findSame(running, key)
  if (same !== undefined) {
    return { ...item, running: true, disabled: true, description: formatOperationBlocker(same) }
  }
  if (item.disabled === true) {
    return item
  }
  const blocker = getOperationBlocker(running, key)
  return blocker === null ? item : { ...item, disabled: true, description: formatOperationBlocker(blocker) }
}

/**
 * Runs `run` as the operation `spec`, from its start until its promise settles, which for a rewrite includes the sync
 * that shows its result. A second start of the same action (a double click, the same item in another surface) joins the
 * running one and resolves with it; a start that must wait for another operation (`getOperationBlocker`) runs nothing,
 * tells `onBlocked` why and resolves to `undefined`.
 */
export function runOperation<T>(spec: OperationSpec, run: () => Promise<T>, onBlocked: (reason: string) => void = () => {}): Promise<T | undefined> {
  const { running } = operationsStore.getState()
  const same = findSame(running, spec)
  if (same !== undefined) {
    return promises.get(same.key) as Promise<T>
  }
  const blocker = getOperationBlocker(running, spec)
  if (blocker !== null) {
    onBlocked(formatOperationBlocker(blocker))
    return Promise.resolve(undefined)
  }

  const operation: Operation = {
    key: nextKey++,
    id: spec.id,
    subject: spec.subject,
    objects: spec.objects,
    kind: spec.kind,
    label: spec.label,
    repositoryPath: repositoryOf(spec),
    surface: spec.surface ?? null,
    ...(spec.move === undefined ? null : { move: spec.move }),
  }
  operationsStore.setState(state => ({ running: [...state.running, operation] }))
  // the promise is known before `run` starts, so a start of the same action from inside it joins it; `run` starts at
  // once, as a direct call would, and a throw before its first await rejects the promise
  let settle!: { resolve: (value: T) => void; reject: (error: unknown) => void }
  const promise = new Promise<T>((resolve, reject) => {
    settle = { resolve, reject }
  }).finally(() => {
    promises.delete(operation.key)
    operationsStore.setState(state => ({ running: state.running.filter(it => it !== operation) }))
  })
  promises.set(operation.key, promise)
  try {
    run().then(settle.resolve, settle.reject)
  } catch (error) {
    settle.reject(error)
  }
  return promise
}

// ---- hooks ----

/** The running operations, for a surface that renders the items of a builder (`operationItem`) */
export function useOperations(): Operations {
  return useStore(operationsStore, state => state.running)
}

/**
 * Why the action `key` can't start now (`formatOperationBlocker`), for a button that is no menu item; null while it can,
 * also while it runs itself (a start joins it)
 */
export function useOperationBlocker(key: OperationKey): string | null {
  return useStore(operationsStore, state => {
    if (findSame(state.running, key) !== undefined) {
      return null
    }
    const blocker = getOperationBlocker(state.running, key)
    return blocker === null ? null : formatOperationBlocker(blocker)
  })
}

/** Whether one of the actions `ids` of the subject runs (the pull request dialog while its creation runs) */
export function useSubjectRunning(subject: string, ids: readonly string[]): boolean {
  return useStore(operationsStore, state => state.running.some(operation => operation.subject === subject && ids.includes(operation.id)))
}

function busyOf(running: Operations, name: string): Operation | undefined {
  return running.find(operation => operation.objects.includes(name))
}

/** The operation that works on the branch (or the archived branch) `name`, the first one started; undefined while none does */
export function getObjectBusy(name: string): Operation | undefined {
  return busyOf(operationsStore.getState().running, name)
}

/** The moves of the running operations of the repository (the selected one), in their start order (`OperationSpec.move`) */
export function getOperationMoves(repositoryPath = getSelectedProject()?.path ?? ""): RunningMove[] {
  return operationsStore.getState().running.flatMap(it => (it.move === undefined || it.repositoryPath !== repositoryPath ? [] : [it.move]))
}

/** Whether an operation works on the branch (or the archived branch) `name`: its row is busy */
export function isObjectBusy(name: string): boolean {
  return busyOf(operationsStore.getState().running, name) !== undefined
}

/** The busy state of the operation that works on `name` ("Unstacking tool-events"), null while none does */
export function useObjectBusy(name: string): string | null {
  return useStore(operationsStore, state => busyOf(state.running, name)?.label ?? null)
}

/**
 * The busy state of the operation of the subject `subject` whose start no mounted surface shows (a menu, a key, a toast,
 * a popover closed by the click): the trigger of the object's menu shows its spinner; null otherwise
 */
export function useUnclaimedBusy(subject: string): string | null {
  return useStore(operationsStore, state => state.running.find(operation => operation.subject === subject && !isClaimed(state, operation.surface))?.label ?? null)
}

function isClaimed(state: OperationsState, surface: string | null): boolean {
  return surface !== null && (state.claims[surface] ?? 0) > 0
}

/** The busy state of the operation `surface` started, while it runs: the surface shows its spinner; null otherwise */
export function useSurfaceBusy(surface: string | undefined): string | null {
  return useStore(operationsStore, state => (surface === undefined ? null : (state.running.find(operation => operation.surface === surface)?.label ?? null)))
}

/**
 * Claims the spinner of the operations `surface` starts while the component is mounted; a surface that unmounts (a
 * closed popover) leaves the spinner to the trigger of the object's menu (`useUnclaimedBusy`)
 */
export function useIndicatorClaim(surface: string | undefined): void {
  useEffect(() => {
    if (surface === undefined) {
      return
    }
    changeClaim(surface, 1)
    return () => changeClaim(surface, -1)
  }, [surface])
}

function changeClaim(surface: string, delta: number): void {
  operationsStore.setState(state => {
    const count = (state.claims[surface] ?? 0) + delta
    const others = Object.fromEntries(Object.entries(state.claims).filter(([key]) => key !== surface))
    return { claims: count > 0 ? { ...others, [surface]: count } : others }
  })
}
