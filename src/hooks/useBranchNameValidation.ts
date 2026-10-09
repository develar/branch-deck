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
const invalidCharsState: BranchNameValidationState = { color: "error", message: "Use letters, numbers, -, _, . (cannot start with .)", textClass: "text-destructive", isValid: false }

export function sanitizeBranchName(branchName: string): string {
  return branchName.trim().replace(/\s+/g, "-")
}

export function validateBranchName(branchName: string): BranchNameValidationState {
  if (!branchName) {
    return neutralState
  }

  if (sanitizeBranchName(branchName).length < 2) {
    return tooShortState
  }

  // spaces are allowed in the input - they are converted to hyphens; dots are allowed by git branch name rules
  if (!/^[a-zA-Z0-9-_. ]+$/.test(branchName)) {
    return invalidCharsState
  }

  // git rule: cannot start with a dot
  if (branchName.trim().startsWith(".")) {
    return invalidCharsState
  }

  return validState
}

/** Validates a user-entered branch name. Pure and cheap, so it is recomputed on every render. */
export function useBranchNameValidation(branchName: string): BranchNameValidation {
  const validationState = validateBranchName(branchName)
  return {
    sanitizedBranchName: sanitizeBranchName(branchName),
    validationState,
    isValid: validationState.isValid,
  }
}
