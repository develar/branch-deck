import { useEffect, useState } from "react"
import { commands } from "@/lib/bindings"
import { patronusRequest } from "@/stores/patronus"

/**
 * By repository and branch: whether the rules of the branch dismiss stale reviews on push, asked once a session (the
 * rules of a repository rarely change). A failed answer (no token, no access) is no warning and is asked again next time.
 */
const answers = new Map<string, Promise<boolean>>()

function readDismissesStaleReviews(repositoryPath: string, branch: string): Promise<boolean> {
  const key = `${repositoryPath}\n${branch}`
  const cached = answers.get(key)
  if (cached !== undefined) {
    return cached
  }
  const forget = (): boolean => {
    answers.delete(key)
    return false
  }
  const answer = commands.getBranchRules({ repositoryPath, branch }).then(result => (result.status === "ok" ? result.data.dismissStaleReviewsOnPush : forget()), forget)
  answers.set(key, answer)
  return answer
}

/**
 * Whether a push of new commits dismisses the approvals of a pull request into `branch` (the GitHub rules of the
 * repository), asked when `enabled`: the Safe Push form warns before a push takes the approvals away. Null while
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
