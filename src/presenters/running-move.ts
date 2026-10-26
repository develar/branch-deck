// The page's words of the moves the operations of the session make while they run (`RunningOperation.moves` of the
// view's `operations`, written by `operations::registry`; [Landing](../../docs/design/landing.md)): each the
// `RunningMove` of the node of its row, as the page's own moves are until their families move into Rust (s9).
import type { RunningMove as MoveView, RunningOperation } from "@/lib/bindings"
import type { RunningMove } from "@/lib/running-move"
import { robotRestartMove, robotStartMove } from "@/lib/running-move"
import { formatReadinessSubject } from "@/lib/whole-stack"

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
