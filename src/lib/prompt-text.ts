/** Text helpers shared by the prompts for an external coding agent (`agent-prompt.ts`, `robot-prompt.ts`) */

/** Rules every hand-off states: Branch Deck must find the same commits, grouped the same way, afterwards */
export const RULES = [
  "Keep every commit message unchanged, including the `(prefix)` at the start of a subject: Branch Deck groups commits by it.",
  "Don't squash, reorder, split or drop commits.",
  "Don't push.",
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
