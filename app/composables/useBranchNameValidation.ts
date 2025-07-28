import type { Ref } from "vue"

interface ValidationState {
  color?: "error"
  message: string
  textClass: string
  isValid: boolean
}

export function useBranchNameValidation(branchName: Ref<string>) {
  // Auto-transform spaces to hyphens and trim
  const sanitizedBranchName = computed(() => {
    return branchName.value.trim().replace(/\s+/g, "-")
  })

  // Validation state
  const validationState = computed<ValidationState>(() => {
    if (!branchName.value) {
      return {
        color: undefined, // Let UInput use default styling
        message: "",
        textClass: "",
        isValid: false,
      }
    }

    // Use sanitized name for length check
    if (sanitizedBranchName.value.length < 2) {
      return {
        color: "error",
        message: "Branch name must be at least 2 characters",
        textClass: "text-error",
        isValid: false,
      }
    }

    // Allow spaces in input - they'll be converted to hyphens
    // Include dots as allowed characters per Git branch name rules
    if (!/^[a-zA-Z0-9-_. ]+$/.test(branchName.value)) {
      return {
        color: "error",
        message: "Use letters, numbers, -, _, . (cannot start with .)",
        textClass: "text-error",
        isValid: false,
      }
    }

    // Git-specific rule: cannot start with a dot
    if (branchName.value.trim().startsWith(".")) {
      return {
        color: "error",
        message: "Use letters, numbers, -, _, . (cannot start with .)",
        textClass: "text-error",
        isValid: false,
      }
    }

    return {
      color: undefined, // Let UInput use default styling
      message: "",
      textClass: "",
      isValid: true,
    }
  })

  const isValid = computed(() => validationState.value.isValid)

  return {
    sanitizedBranchName: readonly(sanitizedBranchName),
    validationState: readonly(validationState),
    isValid: readonly(isValid),
  }
}