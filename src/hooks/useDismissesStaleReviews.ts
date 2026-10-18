import { useEffect, useState } from "react"
import { commands } from "@/lib/bindings"
import { patronusRequest } from "@/stores/patronus"

/**
 * By repository and branch: whether the rules of the branch dismiss stale reviews on push, asked once a session (the
 * rules of a repository rarely change). A failed answer (no token, no access) is no warning and is asked again next time.
 */
const answers = new Map<string, Promise<boolean>>()
/** The answers that arrived, for a decision read outside React (`peekDismissesStaleReviews`) */
const answered = new Map<string, boolean>()

export function readDismissesStaleReviews(repositoryPath: string, branch: string): Promise<boolean> {
  const key = `${repositoryPath}\n${branch}`
  const cached = answers.get(key)
  if (cached !== undefined) {
    return cached
  }
  const forget = (): boolean => {
    answers.delete(key)
    return false
  }
  const answer = commands
    .getBranchRules({ repositoryPath, branch })
    .then(result => (result.status === "ok" ? result.data.dismissStaleReviewsOnPush : forget()), forget)
    .then(dismisses => {
      answered.set(key, dismisses)
      return dismisses
    })
  answers.set(key, answer)
  return answer
}

/** The answer of `readDismissesStaleReviews` if it arrived; null while it is asked or before */
export function peekDismissesStaleReviews(repositoryPath: string, branch: string): boolean | null {
  return answered.get(`${repositoryPath}\n${branch}`) ?? null
}

/** Forgets the answers (tests) */
export function resetDismissesStaleReviews(): void {
  answers.clear()
  answered.clear()
}

/**
 * Whether a push of new commits dismisses the approvals of a pull request into `branch` (the GitHub rules of the
 * repository), asked when `enabled`: the Safe Push popover warns before a push takes the approvals away. Null while
 * `enabled` and not answered yet, so the form can wait for it instead of reading "keeps them"; false when not `enabled`.
 */
export function useDismissesStaleReviews(branch: string | null, enabled: boolean): boolean | null {
  const [answer, setAnswer] = useState<{ branch: string; dismisses: boolean } | null>(null)
  useEffect(() => {
    if (!enabled || branch === null) {
      return
    }
    let current = true
    void readDismissesStaleReviews(patronusRequest().repositoryPath, branch).then(dismisses => {
      if (current) {
        setAnswer({ branch, dismisses })
      }
    })
    return () => {
      current = false
    }
  }, [branch, enabled])
  if (!enabled) {
    return false
  }
  return branch !== null && answer?.branch === branch ? answer.dismisses : null
}
