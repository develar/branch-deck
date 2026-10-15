// Shared branch utilities
import type { SubjectParts } from "@/lib/commit"
import { splitSubjectPrefix } from "@/lib/commit"

export function getSimpleBranchName(fullName: string): string {
  const lastSlash = fullName.lastIndexOf("/")
  return lastSlash === -1 ? fullName : fullName.slice(lastSlash + 1)
}

/**
 * What a branch row and the branch preview show after the name, null for nothing: the title of the branch's description
 * as written, else its summary (a subject) with the prefix split off, to be dimmed
 */
export function getBranchSummary(summary: string, descriptionTitle: string | null): SubjectParts | null {
  if (descriptionTitle !== null) {
    return { prefix: null, text: descriptionTitle }
  }
  return summary === "" ? null : splitSubjectPrefix(summary)
}
