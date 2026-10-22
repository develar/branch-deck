import { useEffect, useState } from "react"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { patronusRequest } from "@/stores/patronus"

/**
 * What the GitHub rules of a branch do to the approvals of a pull request into it when new commits are pushed: `keep`,
 * `dismiss` (stale reviews dismissed on push), or `unknown` when they can't be read (no token, no access, GitHub
 * failed), with the reason. Unknown is never "keeps them": a start that pushes into an approved pull request confirms it.
 */
export type BranchRulesAnswer = "keep" | "dismiss" | { readonly kind: "unknown"; readonly reason: string }

/** The rules couldn't be read */
export function isRulesUnknown(answer: BranchRulesAnswer): answer is { readonly kind: "unknown"; readonly reason: string } {
  return typeof answer === "object"
}

/**
 * By repository and branch: the read of the rules, kept for the session once it answered `keep` or `dismiss` (the rules
 * of a repository rarely change). A failed read is not kept: the next read asks GitHub again, and each start of a robot
 * forgets it first (`forgetUnknownRules`), so it waits for that read.
 */
const answers = new Map<string, Promise<BranchRulesAnswer>>()
/** The answers that arrived, for a decision read outside React (`peekDismissesStaleReviews`); a new ask clears an unknown */
const answered = new Map<string, BranchRulesAnswer>()

export function readDismissesStaleReviews(repositoryPath: string, branch: string): Promise<BranchRulesAnswer> {
  const key = `${repositoryPath}\n${branch}`
  const cached = answers.get(key)
  if (cached !== undefined) {
    return cached
  }
  // asked again after a failed read: the decision waits for this answer, not for the old reason
  answered.delete(key)
  // the reason ends a clause of the popover's line: without its own final period
  const unknown = (error: unknown): BranchRulesAnswer => ({ kind: "unknown", reason: getErrorDetails(error).replace(/\.$/, "") })
  const answer: Promise<BranchRulesAnswer> = commands
    .getBranchRules({ repositoryPath, branch })
    .then(result => (result.status === "ok" ? (result.data.dismissStaleReviewsOnPush ? "dismiss" : "keep") : unknown(result.error)), unknown)
    .then(rules => {
      answered.set(key, rules)
      if (isRulesUnknown(rules) && answers.get(key) === answer) {
        answers.delete(key)
      }
      return rules
    })
  answers.set(key, answer)
  return answer
}

/** The answer of `readDismissesStaleReviews` if it arrived; null while it is asked or before */
export function peekDismissesStaleReviews(repositoryPath: string, branch: string): BranchRulesAnswer | null {
  return answered.get(`${repositoryPath}\n${branch}`) ?? null
}

/** Forgets the failed reads that arrived, so the next start waits for a new read instead of the old reason */
export function forgetUnknownRules(): void {
  for (const [key, rules] of answered) {
    if (isRulesUnknown(rules)) {
      answered.delete(key)
    }
  }
}

/** Forgets the answers (tests) */
export function resetDismissesStaleReviews(): void {
  answers.clear()
  answered.clear()
}

/**
 * What a push of new commits does to the approvals of a pull request into `branch` (the GitHub rules of the
 * repository), asked when `enabled`: the Request Review form warns before a push takes the approvals away. Null while
 * `enabled` and not answered yet; `keep` when not `enabled`. Each form asks again after a failed read.
 */
export function useDismissesStaleReviews(branch: string | null, enabled: boolean): BranchRulesAnswer | null {
  const [answer, setAnswer] = useState<{ branch: string; rules: BranchRulesAnswer } | null>(null)
  useEffect(() => {
    if (!enabled || branch === null) {
      return
    }
    let current = true
    void readDismissesStaleReviews(patronusRequest().repositoryPath, branch).then(rules => {
      if (current) {
        setAnswer({ branch, rules })
      }
    })
    return () => {
      current = false
    }
  }, [branch, enabled])
  if (!enabled) {
    return "keep"
  }
  return branch !== null && answer?.branch === branch ? answer.rules : null
}
