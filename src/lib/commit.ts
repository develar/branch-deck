import type { Commit, MissingCommit } from "@/lib/bindings"
import type { SyncedCommit } from "@/stores/branch-sync"

/** Any commit shape displayed by the commit list. */
export type CommitUnion = Commit | SyncedCommit | MissingCommit

/** Original hash for `Commit`/`SyncedCommit`, `hash` for `MissingCommit`. */
export function getCommitHash(commit: CommitUnion): string {
  if ("originalHash" in commit && commit.originalHash) {
    return commit.originalHash
  }
  if ("hash" in commit && commit.hash) {
    return commit.hash
  }
  return ""
}

/** Subject line to display: stripped subject (without the grouping prefix), else the subject, else the whole message. */
export function getCommitSubject(commit: CommitUnion): string {
  if ("strippedSubject" in commit && commit.strippedSubject) {
    return commit.strippedSubject
  }
  if ("subject" in commit && commit.subject) {
    return commit.subject
  }
  return commit.message
}

/** Body of a multi-line message: everything after the subject line, without the blank lines that follow it. */
export function getCommitMessageBody(message: string): string {
  const lines = message.split("\n")
  let bodyStartIndex = 1
  while (bodyStartIndex < lines.length && lines[bodyStartIndex]?.trim() === "") {
    bodyStartIndex++
  }
  return lines.slice(bodyStartIndex).join("\n").trim()
}

/** Whether the message has a body beyond its subject line. */
export function hasCommitMessageBody(subject: string, message: string): boolean {
  return !!message && message !== subject && message.includes("\n")
}

// conventional-commit prefix: "feat:", "feat(model):", "fix!:", or a type and one scope word ("docs air:")
const SUBJECT_PREFIX = /^([a-z][a-z0-9-]*(?:\([^)]+\)| [a-z][\w./-]*)?!?:)\s+(\S.*)$/
// Branch Deck grouping prefix of a commit that is not stripped by the backend (missing commits): "(feature-auth) …"
const GROUPING_PREFIX = /^\(([^()\s]+)\)\s+(\S.*)$/

/**
 * Splits a prefix off the subject: a grouping prefix (`(feature-auth) …`, returned without the parentheses) or a
 * conventional-commit prefix (with its colon). `prefix` is null if there is none.
 */
export function splitSubjectPrefix(subject: string): { prefix: string | null, text: string } {
  const match = GROUPING_PREFIX.exec(subject) ?? SUBJECT_PREFIX.exec(subject)
  return match == null ? { prefix: null, text: subject } : { prefix: match[1]!, text: match[2]! }
}
