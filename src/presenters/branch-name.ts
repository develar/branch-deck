import type { BranchNameRules, BranchNameState } from "@/lib/bindings"

/** How a typed branch name shows under its field */
export interface BranchNameValidationState {
  /** `"error"` if the input should be shown as invalid (`aria-invalid`) */
  color?: "error"
  message: string
  textClass: string
  isValid: boolean
}

const neutralState: BranchNameValidationState = { color: undefined, message: "", textClass: "", isValid: false }
const validState: BranchNameValidationState = { color: undefined, message: "", textClass: "", isValid: true }

function invalid(message: string): BranchNameValidationState {
  return { color: "error", message, textClass: "text-destructive", isValid: false }
}

/**
 * The words of what Rust found of a typed branch name (`check_branch_name`, `BranchNameState`), by the rules it checked
 * it by: the characters a name may have say that a `/` separates a parent from its branch with stacked branches on
 */
export function presentBranchNameState(state: BranchNameState, rules: BranchNameRules = {}): BranchNameValidationState {
  switch (state) {
    case "empty":
      return neutralState
    case "valid":
      return validState
    case "tooShort":
      return invalid("Branch name must be at least 2 characters")
    case "invalidChars":
      return invalid(
        rules.stacks === true
          ? "Use letters, numbers, -, _, . and / between a parent and its branch (cannot start with .)"
          : "Use letters, numbers, -, _, ., / (cannot start with .)",
      )
    case "leafOnly":
      return invalid("A branch name has no /: use Stack On… to change its parent")
    case "invalidRef":
      return invalid("Not a valid branch name: no -, . or / at the ends, no -- , .. or //")
  }
}
