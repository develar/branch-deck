// The page's words of the moves the operations of the session make while they run (`RunningOperation.moves` of the
// view's `operations`, written by `operations::registry`; [Landing](../../docs/design/landing.md)): each the
// `RunningMove` of the node of its row, as the page's own operations declare theirs until their families move into
// Rust (s9: the restart of a robot, the creation of a pull request, a review request).
import type { RunningMove as MoveView, PatronusPushMode, RunningOperation } from "@/lib/bindings"
import { formatReadinessSubject } from "@/lib/whole-stack"

/**
 * An action of the user that is the move of a row while it runs: the start or the restart of a robot, the creation of
 * a pull request, a review request. The move is made when the user makes it: the node of its row leaves the user's
 * moves at once, not when GitHub or Patronus answer.
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

/** A start of a robot: a Safe Push comes to land the row, a Dry Run only checks it */
function robotStartMove(key: string, mode: PatronusPushMode): RunningMove {
  return robotMove(key, mode, "Landing: Safe Push starting", "Dry Run starting")
}

/** A restart of a robot, with the push before it when the branch moved past its commit */
export function robotRestartMove(key: string, mode: PatronusPushMode): RunningMove {
  return robotMove(key, mode, "Landing: Safe Push restarting", "Dry Run restarting")
}

/** A move of the view in the page's words */
export function presentRunningMove(move: MoveView): RunningMove {
  switch (move.kind) {
    case "starting":
      return robotStartMove(move.subject, move.mode)
    case "restarting":
      return robotRestartMove(move.subject, move.mode)
    case "creatingPullRequest":
      return { key: move.subject, lands: false, label: `Creating the pull request of ${formatReadinessSubject(move.subject)}` }
    case "requestingReview":
      return { key: move.subject, lands: false, label: `Requesting a review on #${move.number}` }
  }
}

/** The moves of the operations of the session, operation by operation in the order of their ids */
export function presentOperationMoves(operations: Readonly<Record<string, RunningOperation>>): RunningMove[] {
  return Object.values(operations).flatMap(operation => operation.moves.map(presentRunningMove))
}
