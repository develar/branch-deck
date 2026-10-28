import { useDebouncedRead } from "@/hooks/useDebouncedRead"
import type { BranchNameCheck, BranchNameRules } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { presentBranchNameState } from "@/presenters/branch-name"
import type { BranchNameValidationState } from "@/presenters/branch-name"

/** A typed name as Rust checked it, worded */
export interface CheckedBranchName {
  /** The name the branch gets: trimmed, its runs of whitespace turned into `-` */
  sanitizedBranchName: string
  validationState: BranchNameValidationState
  isValid: boolean
}

export interface BranchNameValidation extends CheckedBranchName {
  /** Rust has not answered for the name as it is typed now: the answer of an earlier name shows meanwhile */
  pending: boolean
  /** The check of the name as typed now, asked at once when Rust has not answered yet: a submit waits for it */
  settled: () => Promise<CheckedBranchName>
}

const NOTHING_TYPED: CheckedBranchName = { sanitizedBranchName: "", validationState: presentBranchNameState("empty"), isValid: false }

function checked(check: BranchNameCheck | undefined, rules: BranchNameRules): CheckedBranchName {
  if (check === undefined) {
    return NOTHING_TYPED
  }
  const validationState = presentBranchNameState(check.state, rules)
  return { sanitizedBranchName: check.sanitized, validationState, isValid: validationState.isValid }
}

/**
 * Checks a user-entered branch name in Rust (`validate_branch_name`), once the typing pauses; the answer of each name is
 * kept, so typing back shows it at once
 */
export function useBranchNameValidation(branchName: string, rules: BranchNameRules = {}): BranchNameValidation {
  const stacks = rules.stacks === true
  const leafOnly = rules.leafOnly === true
  const ask = (name: string) => commands.validateBranchName({ name, rules: { stacks, leafOnly } })
  const read = useDebouncedRead(branchName === "" ? null : branchName, ask, `${stacks}\n${leafOnly}`)
  const shown = branchName === "" ? NOTHING_TYPED : checked(read.answer, rules)
  return {
    ...shown,
    // a name the answer is not for yet is no name to submit
    isValid: shown.isValid && !read.pending,
    pending: read.pending,
    settled: async () => (branchName === "" ? NOTHING_TYPED : checked(await read.settled(), rules)),
  }
}
