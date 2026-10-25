import type { BranchRules, Fact } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

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

const unknownRules = new WeakMap<Fact<BranchRules>, BranchRulesAnswer>()

/**
 * The answer of the rules of a base as the session read them (`facts.rules`); null while it has not. One object per
 * fact, so a selector returns the same answer for the same fact
 */
export function rulesAnswerOf(fact: Fact<BranchRules> | undefined): BranchRulesAnswer | null {
  if (fact === undefined) {
    return null
  }
  if (fact.state.kind === "unavailable") {
    let answer = unknownRules.get(fact)
    if (answer === undefined) {
      // the reason ends a clause of the popover's line: without its own final period
      answer = { kind: "unknown", reason: getErrorDetails(fact.state.reason).replace(/\.$/, "") }
      unknownRules.set(fact, answer)
    }
    return answer
  }
  if (fact.value === null) {
    return null
  }
  return fact.value.dismissStaleReviewsOnPush ? "dismiss" : "keep"
}
