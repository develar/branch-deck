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
