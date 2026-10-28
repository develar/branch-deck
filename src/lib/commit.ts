import type { Commit, MissingCommit, ReviewCommit } from "@/lib/bindings"
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

/**
 * What a row shows of the subject: `getCommitSubject` without the autosquash markers, which Rust split off
 * (`subjectParts.autosquash`) and a row shows apart
 */
export function getShownSubject(commit: CommitUnion): string {
  const autosquash = commit.subjectParts.autosquash
  const subject = getCommitSubject(commit)
  return autosquash !== "" && subject.startsWith(autosquash) ? subject.slice(autosquash.length) : subject
}

/** Whether the message has a body beyond its subject line. */
export function hasCommitMessageBody(subject: string, message: string): boolean {
  return !!message && message !== subject && message.includes("\n")
}

// conventional-commit prefix: "feat:", "feat(model):", "fix!:", or a type and one scope word ("docs air:")
const SUBJECT_PREFIX = /^([a-z][a-z0-9-]*(?:\([^)]+\)| [a-z][\w./-]*)?!?:)\s+(\S.*)$/
// Branch Deck grouping prefix of a commit that is not stripped by the backend (missing commits): "(feature-auth) …"
const GROUPING_PREFIX = /^\(([^()\s]+)\)\s+(\S.*)$/

/** A subject with its prefix split off; `prefix` is null if there is none */
export interface SplitSubject {
  prefix: string | null
  text: string
}

/**
 * Splits a prefix off the subject: a grouping prefix (`(feature-auth) …`, returned without the parentheses) or a
 * conventional-commit prefix (with its colon). `prefix` is null if there is none.
 */
export function splitSubjectPrefix(subject: string): SplitSubject {
  const match = GROUPING_PREFIX.exec(subject) ?? SUBJECT_PREFIX.exec(subject)
  return match == null ? { prefix: null, text: subject } : { prefix: match[1]!, text: match[2]! }
}

/** "1 commit", "3 commits" */
export function formatCommitCount(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`
}

/** Number of changed files, if the commit shape carries it. */
export function getCommitFileCount(commit: CommitUnion): number | undefined {
  if ("filesTouched" in commit && commit.filesTouched.length > 0) {
    return commit.filesTouched.length
  }
  if ("fileCount" in commit && typeof commit.fileCount === "number") {
    return commit.fileCount
  }
  return undefined
}

/** The first line of a message */
export function getSubjectLine(message: string): string {
  return message.trimStart().split("\n", 1)[0]!
}

/** A commit of a list as a review shows it: its id (the original hash), subject, message, author and times */
export function toReviewCommit(commit: CommitUnion): ReviewCommit {
  return {
    id: getCommitHash(commit),
    subject: getCommitSubject(commit),
    message: commit.message,
    author: commit.author,
    authorTime: commit.authorTime,
    committerTime: commit.committerTime,
  }
}
