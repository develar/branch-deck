import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { OperationId } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { getSelectedProject } from "@/stores/repository"

/**
 * What an action does to the repository: a rewrite moves HEAD or a ref of Branch Deck (a regroup, a squash, a delete),
 * which the session of the repository admits and queues behind the rewrites before it (`runRewriteIntent`), the record
 * here only showing it, and Update Project waits for (`whenRewritesDone`); a remote action talks to a server (a pull
 * request, a robot)
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
  /**
   * What a remote action writes, where it is narrower than its subject: a field of a pull request (`pr:8:base`,
   * `pr:8:reviewers`). Two remote actions that both name theirs conflict only when they share one, so a retarget and a
   * review request of one pull request run side by side
   */
  readonly resources?: readonly string[]
  /**
   * What the start acts with, where two starts of one action of one subject do different things: the commits and the
   * target of a move, the commit and the message of a reword, the commits and the message of a squash. A second start
   * joins only the same parameters; one with others is another operation, which waits for it in the queue
   */
  readonly parameters?: string
}

export interface OperationSpec extends OperationKey {
  /** The busy state in sentence case, without the ellipsis: "Unstacking tool-events" */
  readonly label: string
  /**
   * The surface that started it (`useIndicatorClaim`): a button that stays shows the spinner itself; null for a menu, a
   * key or a toast, whose row (the trigger of the object's menu) shows it
   */
  readonly surface?: string | null
}

export interface Operation extends OperationSpec {
  readonly key: number
  readonly repositoryPath: string
  readonly surface: string | null
  /** The busy state of the operation it waits for in the queue ("Unstacking oauth"); null once it runs */
  readonly waitsFor: string | null
  /**
   * Whom its operation waits for while another holder has the repository lock, as the step of its operation says it:
   * "Waiting for `bd pick` (pid 4242, for 12 s)" (`setSessionStep`); null while it waits for nobody
   */
  readonly lockWait: string | null
  /**
   * The operation of the session it shows (a rewrite of the engine), once its channel named it, which its Stop Waiting
   * stops (`presentStopWaiting`); null before and for an operation the page runs itself
   */
  readonly op: OperationId | null
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

/** The running operation that `key` joins: the same action of the same object with the same parameters */
export function findSame(running: Operations, key: OperationKey): Operation | undefined {
  const repositoryPath = repositoryOf(key)
  return running.find(it => it.id === key.id && it.subject === key.subject && it.parameters === key.parameters && it.repositoryPath === repositoryPath)
}

/**
 * The running operation that `key` would wait for, null when it can run: a rewrite for any rewrite of the repository
 * (the session queues them; Update Project waits for them, and says so), a remote action for a remote action of the
 * same object or of a branch it works on, or of a resource it writes when both name theirs (`resources`). A rewrite
 * and a remote action run side by side: a push waits for the sync after the rewrite itself.
 */
export function getOperationBlocker(running: Operations, key: OperationKey): Operation | null {
  const repositoryPath = repositoryOf(key)
  for (const operation of running) {
    if (operation.repositoryPath === repositoryPath && operation.kind === key.kind && conflicts(operation, key)) {
      return operation
    }
  }
  return null
}

function conflicts(operation: Operation, key: OperationKey): boolean {
  if (key.kind === "rewrite") {
    return true
  }
  if (operation.resources !== undefined && key.resources !== undefined) {
    return operation.resources.some(resource => key.resources!.includes(resource))
  }
  return operation.subject === key.subject || operation.objects.some(name => key.objects.includes(name))
}

/**
 * The busy state of an operation as its surfaces show it: "Unstacking tool-events", what it waits for in the queue
 * ("Waits for Unstacking oauth"), or whom its command waits for while another holder has the repository lock ("Waiting
 * for `bd pick` (pid 4242, for 12 s)")
 */
export function operationLabel(operation: Operation): string {
  if (operation.lockWait !== null) {
    return operation.lockWait
  }
  return operation.waitsFor === null ? operation.label : `Waits for ${operation.waitsFor}`
}

/**
 * What the operation of the session behind the record `key` (`runRewriteIntent`) waits for, as its channel tells it:
 * whom while another holder has the repository lock (`lockWait`, stopped by `presentStopWaiting` of the operation
 * `op`), the operation before it in the queue of the session (`waitsFor`, its busy state); both null once it runs
 */
export function setSessionStep(key: number, step: { readonly op: OperationId; readonly lockWait: string | null; readonly waitsFor: string | null }): void {
  const changed = (it: Operation) => it.op !== step.op || it.lockWait !== step.lockWait || it.waitsFor !== step.waitsFor
  operationsStore.setState(state => ({ running: state.running.map(it => (it.key === key && changed(it) ? { ...it, ...step } : it)) }))
}

/** The busy state of `operation` with the ellipsis, as an item that runs it says it: "Unstacking tool-events…" */
export function formatOperationBlocker(operation: Operation): string {
  return `${operationLabel(operation)}…`
}

/**
 * Resolves once no rewrite of the repository runs or waits, the sync that shows its result included
 * (`runRewriteIntent`): a push started meanwhile pushes the branches as the rewrite leaves them, an update updates
 * them. Never rejects.
 */
export async function whenRewritesDone(repositoryPath: string): Promise<void> {
  while (true) {
    const rewrite = operationsStore.getState().running.findLast(operation => operation.kind === "rewrite" && operation.repositoryPath === repositoryPath)
    const promise = rewrite === undefined ? undefined : promises.get(rewrite.key)
    if (promise === undefined) {
      return
    }
    await promise.then(
      () => {},
      () => {},
    )
  }
}

/** The running operations; a component reads them with `useOperations` */
export function getOperations(): Operations {
  return operationsStore.getState().running
}

/**
 * The item of an action of an object as the running operations leave it: running while it runs itself (a second start
 * joins it); a remote action that must wait is disabled with the busy state of the operation it waits for, a rewrite
 * stays enabled, its start waits in the queue of the session; else as it is. An item already disabled keeps its reason;
 * a submenu (Stack On ›) is disabled as a whole.
 */
export function operationItem(running: Operations, item: MenuItem, key: OperationKey): MenuItem {
  const same = findSame(running, key)
  if (same !== undefined) {
    return { ...item, running: true, disabled: true, description: formatOperationBlocker(same) }
  }
  if (item.disabled === true || key.kind === "rewrite") {
    return item
  }
  const blocker = getOperationBlocker(running, key)
  return blocker === null ? item : { ...item, disabled: true, description: formatOperationBlocker(blocker) }
}

/**
 * Runs `run` as the operation `spec`, from its start until its promise settles, which for a rewrite includes the sync
 * that shows its result. A second start of the same action with the same parameters (a double click, the same item in
 * another surface) joins the running one and resolves with it; a start with other parameters (`OperationKey.parameters`:
 * other commits of a move) is an operation of its own. A rewrite starts at once: the session of its repository queues
 * it behind the rewrites before it, and its record says what it waits for as the steps of its operation tell it
 * (`setSessionStep`). A remote action that must wait (a cancel of the robot a restart replaces) runs nothing, tells
 * `onBlocked` why and resolves to `undefined`: it would act on the robot or the pull request as it was before the one
 * it waits for.
 */
export function runOperation<T>(spec: OperationSpec, run: (key: number) => Promise<T>, onBlocked: (reason: string) => void = () => {}): Promise<T | undefined> {
  const { running } = operationsStore.getState()
  const same = findSame(running, spec)
  if (same !== undefined) {
    return promises.get(same.key) as Promise<T>
  }
  const blocker = spec.kind === "remote" ? getOperationBlocker(running, spec) : null
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
    waitsFor: null,
    lockWait: null,
    op: null,
    ...(spec.resources === undefined ? null : { resources: spec.resources }),
    ...(spec.parameters === undefined ? null : { parameters: spec.parameters }),
  }
  operationsStore.setState(state => ({ running: [...state.running, operation] }))
  // the promise is known before `run` starts, so a start of the same action from inside it joins it; `run` starts at
  // once, as a direct call would, and a throw before its first await rejects the promise
  let settle!: { resolve: (value: T) => void; reject: (error: unknown) => void }
  const promise = new Promise<T>((resolve, reject) => {
    settle = { resolve, reject }
  }).finally(() => {
    promises.delete(operation.key)
    operationsStore.setState(state => ({ running: state.running.filter(it => it.key !== operation.key) }))
  })
  promises.set(operation.key, promise)
  try {
    run(operation.key).then(settle.resolve, settle.reject)
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
 * The busy state of the operation a start of `key` would wait for ("Unstacking oauth…"), for a button that is no menu
 * item: a remote action can't start then (the button is disabled with it), a rewrite waits in the queue (its hover says
 * it); null while nothing is in its way, also while it runs itself (a start joins it)
 */
export function useOperationWait(key: OperationKey): string | null {
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

/** Whether an operation works on the branch (or the archived branch) `name`: its row is busy */
export function isObjectBusy(name: string): boolean {
  return busyOf(operationsStore.getState().running, name) !== undefined
}

/** `getObjectBusy`, re-rendering when it changes: the same object while the operation runs */
export function useObjectOperation(name: string): Operation | undefined {
  return useStore(operationsStore, state => busyOf(state.running, name))
}

/** The operations that run while `wanted`, else null: a surface that shows them for one item only re-renders for them */
export function useOperationsWhile(wanted: boolean): Operations | null {
  return useStore(operationsStore, state => (wanted ? state.running : null))
}

/** The busy state of the operation that works on `name` ("Unstacking tool-events"), null while none does */
export function useObjectBusy(name: string): string | null {
  return useStore(operationsStore, state => {
    const operation = busyOf(state.running, name)
    return operation === undefined ? null : operationLabel(operation)
  })
}

/**
 * The operation of the subject `subject` whose start no mounted surface shows (a menu, a key, a toast, a popover closed
 * by the click): the trigger of the object's menu shows its spinner and busy state (`operationLabel`), and its Stop
 * Waiting (`presentStopWaiting`); undefined otherwise
 */
export function useUnclaimedOperation(subject: string): Operation | undefined {
  return useStore(operationsStore, state => state.running.find(it => it.subject === subject && !isClaimed(state, it.surface)))
}

function isClaimed(state: OperationsState, surface: string | null): boolean {
  return surface !== null && (state.claims[surface] ?? 0) > 0
}

/** The busy state of the operation `surface` started, while it runs: the surface shows its spinner; null otherwise */
export function useSurfaceBusy(surface: string | undefined): string | null {
  return useStore(operationsStore, state => {
    const operation = surfaceOf(state, surface)
    return operation === undefined ? null : operationLabel(operation)
  })
}

function surfaceOf(state: OperationsState, surface: string | undefined): Operation | undefined {
  return surface === undefined ? undefined : state.running.find(it => it.surface === surface)
}

/** The operation `surface` started, while it runs, for its Stop Waiting (`presentStopWaiting`); undefined otherwise */
export function useSurfaceOperation(surface: string | undefined): Operation | undefined {
  return useStore(operationsStore, state => surfaceOf(state, surface))
}

/**
 * Claims the spinner of the operations `surface` starts while the component is mounted; a surface that unmounts (a
 * closed popover) leaves the spinner to the trigger of the object's menu (`useUnclaimedOperation`)
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
