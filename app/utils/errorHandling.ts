// Main function with optional toast parameter
export function reportError(title: string, error: unknown, toast?: ReturnType<typeof useToast>): void {
  const toastInstance = toast || useToast()

  // Better error message extraction
  let description: string
  if (error instanceof Error) {
    description = error.message
  }
  else if (typeof error === "string") {
    description = error
  }
  else if (error && typeof error === "object" && "message" in error) {
    description = String(error.message)
  }
  else {
    description = "An unexpected error occurred"
  }

  toastInstance.add({
    color: "error",
    title,
    description,
    duration: 0, // Don't auto-dismiss errors
    progress: false,
  })
}

// Convenience function for internal errors
export function reportInternalError(error: unknown, context?: string): void {
  const title = context ? `Internal Error: ${context}` : "Internal Error"
  reportError(title, error)
}

// Convenience function for user-facing validation errors
export function reportValidationError(message: string): void {
  const toast = useToast()
  toast.add({
    color: "warning",
    title: "Validation Error",
    description: message,
    duration: 5000, // Auto-dismiss after 5 seconds
  })
}
