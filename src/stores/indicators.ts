import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { OperationId, RunningOperation } from "@/lib/bindings"
import type { WaitingOperation } from "@/presenters/lock-wait"
import { presentOperationLockWait } from "@/presenters/lock-wait"
import type { OperationWords } from "@/presenters/operations"
import { presentOperationBusy } from "@/presenters/operations"
import { branchSyncStore, useBranchSyncField } from "@/stores/branch-sync"

/**
 * The spinners of the surfaces that start an operation of the session (docs/design/operations.md, "The indicator"). The
 * page admits and queues nothing: the session does, and every window reads what runs from the view
 * (`RepositoryView.operations`, the actions Rust lays them over). What only this window knows is where an operation
 * started: a surface that stays while its action runs (a button, an inline form, a dialog, a popover) claims its key
 * while it is mounted (`useIndicatorClaim`) and shows the spinner of what it started from the click until its `execute`
 * answered (`startIndicator`, `useSurfaceBusy`), its operation known by its id once the channel named it. An operation
 * whose surface has no mounted claim (a menu, a key, a toast, a popover the click closed) shows its spinner on the "⋯"
 * of its row (`isClaimed`).
 */
interface IndicatorsState {
  /** The surfaces on the page that show the spinner of what they started, by the count of their mounts */
  readonly claims: Readonly<Record<string, number>>
  /** What each surface started, from the click until its `execute` answered */
  readonly runs: Readonly<Record<string, SurfaceRun>>
  /** The surface and the busy state of each operation of the session this window started, by its id */
  readonly starts: Readonly<Record<OperationId, Started>>
}

/** What a surface started while it runs: its busy state, and the operation of the session once its channel named it */
interface SurfaceRun {
  readonly label: string
  readonly op: OperationId | null
}

/** Where an operation this window started came from: its surface (null for a menu, a key, a toast) and its busy state */
interface Started {
  readonly surface: string | null
  readonly label: string
}

export const indicatorsStore = createStore<IndicatorsState>(() => ({ claims: {}, runs: {}, starts: {} }))

/** The indicator of one start: named once the channel of its `execute` names its operation, done once it answered */
export interface Indicator {
  readonly named: (op: OperationId) => void
  readonly done: () => void
}

function without<T>(record: Readonly<Record<string, T>>, key: string): Readonly<Record<string, T>> {
  return Object.fromEntries(Object.entries(record).filter(([it]) => it !== key))
}

/**
 * Records a start of the busy state `label` from `surface` (null: a menu, a key, a toast): the surface shows its spinner
 * until `done`, and its operation, once `named`, is known to have started there
 */
export function startIndicator(surface: string | null | undefined, label: string): Indicator {
  const at = surface ?? null
  const run: SurfaceRun = { label, op: null }
  let current = run
  let op: OperationId | null = null
  if (at !== null) {
    indicatorsStore.setState(state => ({ runs: { ...state.runs, [at]: run } }))
  }
  return {
    named: named => {
      if (op === named) {
        return
      }
      op = named
      indicatorsStore.setState(state => {
        const runs = at !== null && state.runs[at] === current ? { ...state.runs, [at]: (current = { label, op: named }) } : state.runs
        return { runs, starts: { ...state.starts, [named]: { surface: at, label } } }
      })
    },
    done: () =>
      indicatorsStore.setState(state => ({
        runs: at !== null && state.runs[at] === current ? without(state.runs, at) : state.runs,
        starts: op === null ? state.starts : without(state.starts, op),
      })),
  }
}

/** The operations of the session and the busy states this window gave its own (`BlockerWords`), as they are now */
export function getOperationWords(): OperationWords {
  return { operations: branchSyncStore.getState().operations, named: indicatorsStore.getState().starts }
}

/**
 * The busy state of `operation` as its surfaces say it now (`presentOperationBusy` of `getOperationWords`): whom it
 * waits for while another holder has the repository lock, the operation it waits behind in the queue, else its own, each
 * by the name the items give it (`presentOperationLabel`)
 */
export function operationBusyOf(operation: RunningOperation): string {
  return presentOperationBusy(operation, getOperationWords())
}

/** Whether the surface that started `operation` is mounted and shows its spinner: the "⋯" of its row shows it otherwise */
export function isClaimed(operation: RunningOperation): boolean {
  const { starts, claims } = indicatorsStore.getState()
  const surface = starts[operation.id]?.surface ?? null
  return surface !== null && (claims[surface] ?? 0) > 0
}

/** The run that `surface` started, while it runs */
function useSurfaceRun(surface: string | undefined): SurfaceRun | undefined {
  return useStore(indicatorsStore, state => (surface === undefined ? undefined : state.runs[surface]))
}

/** The operation of the view behind the run of `surface`, once named and while the view has it */
function useRunOperation(run: SurfaceRun | undefined): RunningOperation | undefined {
  const op = run?.op ?? null
  return useBranchSyncField("operations", operations => (op === null ? undefined : operations[op]))
}

/**
 * The busy state of what `surface` started, from the click until its `execute` answered ("Squashing 2 commits", "Waits
 * for Unstacking oauth", "Waiting for `bd pick` (pid 4242, for 12 s)"): the surface shows its spinner; null otherwise
 */
export function useSurfaceBusy(surface: string | undefined): string | null {
  const run = useSurfaceRun(surface)
  // a new step of its operation is another object: the words follow it
  const operation = useRunOperation(run)
  if (run === undefined) {
    return null
  }
  return operation === undefined ? run.label : operationBusyOf(operation)
}

/** What `surface` started, while it runs, as Stop Waiting reads it (`presentStopWaiting`); undefined otherwise */
export function useSurfaceOperation(surface: string | undefined): WaitingOperation | undefined {
  const run = useSurfaceRun(surface)
  const operation = useRunOperation(run)
  return run === undefined ? undefined : { op: run.op, lockWait: presentOperationLockWait(operation) }
}

/**
 * Claims the spinner of the operations `surface` starts while the component is mounted; a surface that unmounts (a
 * closed popover) leaves the spinner to the "⋯" of the row of the operation (`isClaimed`)
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
  indicatorsStore.setState(state => {
    const count = (state.claims[surface] ?? 0) + delta
    const others = without(state.claims, surface)
    return { claims: count > 0 ? { ...others, [surface]: count } : others }
  })
}
