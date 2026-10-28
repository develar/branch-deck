import type { Commit, SubjectParts } from "@/lib/bindings"
import type { RemoteStatus, SyncedCommit } from "@/stores/branch-sync"

/** The message without trailing whitespace on a line and without blank lines at the start or the end */
function cleanUp(message: string): string {
  return message
    .split("\n")
    .map(line => line.trimEnd())
    .join("\n")
    .replace(/^\n+|\n+$/g, "")
}

/** A message of a squash with its subject as Rust split it (`subjectParts`) */
export type SquashedMessage = Pick<Commit, "message" | "subjectParts">

/**
 * The message without what groups it: the autosquash markers and the `(prefix)` of its subject, or the issue key the
 * subject starts with if it is grouped by that key (`ABC-1: Add tokens` → `Add tokens`), as Rust split the subject
 */
function withoutMarkers(message: string, { autosquash, prefix, issueKey }: SubjectParts): string {
  const newline = message.indexOf("\n")
  const subject = (newline === -1 ? message : message.slice(0, newline)).slice(autosquash.length)
  let text = subject
  if (prefix !== null) {
    text = subject.slice(prefix.length + 2).trimStart()
  } else if (issueKey !== null && subject.startsWith(issueKey)) {
    text = subject.slice(issueKey.length).replace(/^[\s:]+/, "")
  }
  return text + (newline === -1 ? "" : message.slice(newline))
}

/**
 * The initial message of commits squashed together, given oldest first: the oldest message as is (its subject keeps
 * the branch), then the others as paragraphs, without their autosquash markers and `(prefix)` or leading issue key. A message already
 * included, like `fixup! Add login` after `Add login`, is left out.
 */
export function buildSquashMessage(commits: readonly SquashedMessage[]): string {
  const [oldest, ...newer] = commits.map(commit => ({ message: cleanUp(commit.message), parts: commit.subjectParts }))
  if (oldest === undefined) {
    return ""
  }
  const parts = [oldest.message]
  const included = new Set([cleanUp(withoutMarkers(oldest.message, oldest.parts))])
  for (const { message, parts: subject } of newer) {
    const text = cleanUp(withoutMarkers(message, subject))
    if (text !== "" && !included.has(text)) {
      parts.push(text)
      included.add(text)
    }
  }
  return parts.join("\n\n")
}

/** Number of `commits` of a branch that are already on its remote branch: squashing them makes the next push replace them */
export function countPushedCommits(commits: readonly (Commit | SyncedCommit)[], remoteStatus: RemoteStatus | null | undefined): number {
  if (remoteStatus?.exists !== true) {
    return 0
  }
  // the remote branch holds the synced commits; one not synced yet can't be on it
  const unpushed = new Set(remoteStatus.unpushedCommits)
  return commits.filter(commit => "hash" in commit && commit.hash !== undefined && !unpushed.has(commit.hash)).length
}
