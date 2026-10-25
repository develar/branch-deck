/** Text helpers shared by the prompts for an external coding agent (`agent-prompt.ts`, `robot-prompt.ts`) */

const MESSAGES = "Keep every commit message unchanged, including the `(prefix)` at the start of a subject: Branch Deck groups commits by it."
const NO_PUSH = "Don't push."

/** Rules every hand-off states: Branch Deck must find the same commits, grouped the same way, afterwards */
export const RULES = [MESSAGES, "Don't squash, reorder, split or drop commits.", NO_PUSH]

/**
 * The rules of an update with bd (`conflictPromptWithBd`): bd drops a commit it proves landed, and `--landed` drops one
 * after the user's yes, so only the agent's own drop is forbidden. A hand-off without bd (a rebase in a terminal, the
 * fix of a failed robot) keeps `RULES`
 */
export const UPDATE_RULES = [
  MESSAGES,
  "Don't squash, reorder or split commits, and don't drop a commit yourself: bd drops a commit it proves landed, and `--landed` drops one after my yes.",
  NO_PUSH,
]

export function bullets(lines: readonly string[]): string {
  return lines.map(line => `- ${line}`).join("\n")
}

export function numbered(lines: readonly string[]): string {
  return lines.map((line, index) => `${index + 1}. ${line}`).join("\n")
}

/**
 * A word for the shell of `bd`, as bd quotes the repository after `-C`: as it is when plain, else single-quoted, with a
 * quote as `'\''` for a POSIX shell and doubled for PowerShell
 */
export function shellWord(word: string, powershell: boolean): string {
  if (/^[\w\-./:@+]+$/.test(word)) {
    return word
  }
  return `'${word.replaceAll("'", powershell ? "''" : "'\\''")}'`
}
