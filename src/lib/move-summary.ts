import type { MoveChange, MoveCommitsResult } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"

/** The commits a move changes: those not already where they go */
export function countMoved(result: MoveCommitsResult): number {
  return result.commits.filter(commit => commit.change !== "Unchanged").length
}

function countChange(result: MoveCommitsResult, change: MoveChange): number {
  return result.commits.filter(commit => commit.change === change).length
}

/** "IJPL-1", "IJPL-1 and IJPL-2", "IJPL-1, IJPL-2 and IJPL-3" */
export function formatKeys(keys: readonly string[]): string {
  return keys.length <= 1 ? keys.join("") : `${keys.slice(0, -1).join(", ")} and ${keys.at(-1)!}`
}

/**
 * What a move does besides "N commits to X", for the "Move to…" form (`done` false: what it will do) and its toast
 * (`done`): the issues that go to the target by their key, without a rewrite, and the commits whose prefix changes,
 * as the second half of a sentence. `told` is the change the title says already, left out here. Undefined when there
 * is nothing to add.
 */
export function describeMove(result: MoveCommitsResult, target: string | null, done: boolean, told: MoveChange | null = null): string | undefined {
  const parts: string[] = []
  const grouped = result.groupedIssues
  if (grouped.length > 0 && target !== null) {
    parts.push(`${formatKeys(grouped)} ${grouped.length === 1 ? "goes" : "go"} to ${target} by the issue key from now on, without a rewrite`)
  }
  const ungrouped = result.ungroupedIssues
  if (ungrouped.length > 0) {
    parts.push(`${formatKeys(ungrouped)} ${ungrouped.length === 1 ? "is back in its issue branch" : "are back in their issue branches"}, without a rewrite`)
  }
  const prefixed = countChange(result, "Prefixed")
  if (prefixed > 0) {
    // back to the issue branch of its key, out of the issue group of the key
    const prefix = target === null ? "the prefix of its issue branch" : `(${target})`
    parts.push(done ? `${formatCommitCount(prefixed)} got ${target === null ? prefix : `the prefix ${prefix}`}` : `rewrites ${formatCommitCount(prefixed)} to start with ${prefix}`)
  }
  const removed = countChange(result, "PrefixRemoved")
  if (removed > 0 && told !== "PrefixRemoved") {
    parts.push(`${done ? "removed" : "removes"} the prefix of ${formatCommitCount(removed)}`)
  }
  if (parts.length === 0) {
    return undefined
  }
  return parts.join("; ")
}

/** The title and the description of a toast */
export interface ToastText {
  readonly title: string
  readonly description: string | undefined
}

/**
 * The toast of a move of `count` commits (`countMoved`) to `target`, or back (null): the title says what it did, the
 * description only the rest (`describeMove`), so a move back that only removed prefixes says it once, in the title
 */
export function describeDoneMove(result: MoveCommitsResult, target: string | null, count: number): ToastText {
  const commits = formatCommitCount(count)
  const onlyPrefixes = target === null && result.commits.every(commit => commit.change === "PrefixRemoved" || commit.change === "Unchanged")
  const title = target !== null ? `Moved ${commits} to ${target}` : onlyPrefixes ? `Removed the prefix of ${commits}` : `Moved ${commits} back`
  const description = describeMove(result, target, true, onlyPrefixes ? "PrefixRemoved" : null)
  return { title, description: description === undefined ? undefined : capitalize(description) }
}

/** The text with a capital first letter, as a toast description starts */
export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}
