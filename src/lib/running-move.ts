import type { PatronusPushMode } from "@/lib/bindings"
import { formatReadinessSubject } from "@/lib/whole-stack"
import type { SafePushStarting } from "@/stores/safe-push"

/**
 * An action of the user that is the move of a row while it runs ([Landing](../../docs/design/landing.md)): the start or
 * the restart of a robot, the creation of a pull request, a review request. The move is made when the user
 * makes it: the node of its row leaves the user's moves at once, not when GitHub or Patronus answer. An operation of the
 * registry declares it (`OperationSpec.move`), a running start of a Safe Push is one (`safePushStartMoves`), and so is
 * a move an operation of the session makes (`RunningOperation.moves`, `src/presenters/running-move.ts`).
 */
export interface RunningMove {
  /** The row whose move it is: a branch, or the whole stack of a root (`stackReadinessKey`) */
  readonly key: string
  /**
   * A robot comes that lands the row, with the branches its Safe Push lands beside it (the ancestors, the members of the
   * whole stack): Landing; false: the move is made and others answer next (the reviewers, GitHub, a Dry Run): Waiting
   */
  readonly lands: boolean
  /** The hover of the node meanwhile: "Landing: Safe Push starting", "Requesting a review on #1078" */
  readonly label: string
}

/** The running move that decides the node of a row */
export interface FoundMove {
  readonly move: RunningMove
  /** The subject of a move of another row that lands this one: "oauth", "the whole air stack"; null for its own */
  readonly with: string | null
}

/**
 * The move that decides the node of `name` among the running ones: its own, else a move that lands it, of a row whose
 * Safe Push lands it beside (`landsBeside`: the ancestors of a branch, the members of a whole stack)
 */
export function findRunningMove(name: string, moves: readonly RunningMove[], landsBeside: (key: string) => readonly string[]): FoundMove | null {
  const own = moves.find(move => move.key === name)
  if (own !== undefined) {
    return { move: own, with: null }
  }
  const landing = moves.find(move => move.lands && landsBeside(move.key).includes(name))
  return landing === undefined ? null : { move: landing, with: formatReadinessSubject(landing.key) }
}

/** What a robot of `mode` comes to do: land the row, or only check it */
function robotMove(key: string, mode: PatronusPushMode, landing: string, checking: string): RunningMove {
  return mode === "dryRun" ? { key, lands: false, label: `${checking}: it checks the branch, it lands nothing` } : { key, lands: true, label: landing }
}

/**
 * The moves of the running starts of a Safe Push or a Dry Run (`SafePushState.starting`, by branch or by
 * `stackReadinessKey`), past their checks: the target check, the push, the start. At its checks a start may still ask
 * in its popover, and nothing started yet.
 */
export function safePushStartMoves(starting: Readonly<Record<string, SafePushStarting>>): RunningMove[] {
  return Object.entries(starting).flatMap(([key, start]) => (start.step === "checking" ? [] : [robotStartMove(key, start.mode)]))
}

/** A start of a robot: a Safe Push comes to land the row, a Dry Run only checks it */
export function robotStartMove(key: string, mode: PatronusPushMode): RunningMove {
  return robotMove(key, mode, "Landing: Safe Push starting", "Dry Run starting")
}

/** A restart of a robot, with the push before it when the branch moved past its commit */
export function robotRestartMove(key: string, mode: PatronusPushMode): RunningMove {
  return robotMove(key, mode, "Landing: Safe Push restarting", "Dry Run restarting")
}
