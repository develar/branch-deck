import { sanitizeBranchName as sanitizeGitBranchName } from "@/lib/commit"

export interface BranchNameValidationState {
  /** `"error"` if the input should be shown as invalid (`aria-invalid`) */
  color?: "error"
  message: string
  textClass: string
  isValid: boolean
}

export interface BranchNameValidation {
  /** Trimmed name with whitespace runs converted to hyphens */
  sanitizedBranchName: string
  validationState: BranchNameValidationState
  isValid: boolean
}

const neutralState: BranchNameValidationState = { color: undefined, message: "", textClass: "", isValid: false }
const validState: BranchNameValidationState = { color: undefined, message: "", textClass: "", isValid: true }
const tooShortState: BranchNameValidationState = { color: "error", message: "Branch name must be at least 2 characters", textClass: "text-destructive", isValid: false }
const invalidCharsState: BranchNameValidationState = {
  color: "error",
  message: "Use letters, numbers, -, _, ., / (cannot start with .)",
  textClass: "text-destructive",
  isValid: false,
}
const invalidStackCharsState: BranchNameValidationState = {
  color: "error",
  message: "Use letters, numbers, -, _, . and / between a parent and its branch (cannot start with .)",
  textClass: "text-destructive",
  isValid: false,
}
const leafOnlyState: BranchNameValidationState = {
  color: "error",
  message: "A branch name has no /: use Stack On… to change its parent",
  textClass: "text-destructive",
  isValid: false,
}
const invalidRefState: BranchNameValidationState = {
  color: "error",
  message: "Not a valid branch name: no -, . or / at the ends, no -- , .. or //",
  textClass: "text-destructive",
  isValid: false,
}

export function sanitizeBranchName(branchName: string): string {
  return branchName.trim().replace(/\s+/g, "-")
}

export interface BranchNameRules {
  /** Stacked branches are on: a `/` separates a parent from its branch */
  stacks?: boolean
  /** Only a branch name, not `parent/branch` (with stacks on) */
  leafOnly?: boolean
}

export function validateBranchName(branchName: string, rules: BranchNameRules = {}): BranchNameValidationState {
  if (!branchName) {
    return neutralState
  }
  if (rules.stacks === true && rules.leafOnly === true && branchName.includes("/")) {
    return leafOnlyState
  }

  if (sanitizeBranchName(branchName).length < 2) {
    return tooShortState
  }

  // spaces are allowed in the input - they are converted to hyphens; dots and slashes are allowed by git branch name rules
  if (!/^[a-zA-Z0-9-_. /]+$/.test(branchName)) {
    return rules.stacks === true ? invalidStackCharsState : invalidCharsState
  }

  // git rule: cannot start with a dot
  if (branchName.trim().startsWith(".")) {
    return rules.stacks === true ? invalidStackCharsState : invalidCharsState
  }

  // the grouper would rename it (`sanitizeGitBranchName`), and the virtual branch would not get this name
  const name = sanitizeBranchName(branchName)
  if (sanitizeGitBranchName(name) !== name || name.includes("//") || name.includes("..")) {
    return invalidRefState
  }

  return validState
}

/** Validates a user-entered branch name. Pure and cheap, so it is recomputed on every render. */
export function useBranchNameValidation(branchName: string, rules: BranchNameRules = {}): BranchNameValidation {
  const validationState = validateBranchName(branchName, rules)
  return {
    sanitizedBranchName: sanitizeBranchName(branchName),
    validationState,
    isValid: validationState.isValid,
  }
}
