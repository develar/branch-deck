import { error as logErrorToBackend } from "@tauri-apps/plugin-log"

export function getErrorDetails(error: unknown) {
  if (error instanceof Error) {
    return error.toString()
  }
  else {
    return String(error)
  }
}

// Main function with optional toast parameter
export function notifyError(title: string, error: unknown, toast?: ReturnType<typeof useToast>): void {
  const toastInstance = toast || useToast()

  // Better error message extraction
  const description = getErrorDetails(error)
  logError(title, error, description)

  toastInstance.add({
    color: "error",
    title,
    description,
    duration: 0, // don't auto-dismiss errors
    progress: false,
  })
}

// Convenience function for internal errors
export function notifyInternalError(error: unknown, context?: string): void {
  const title = context ? `Internal Error: ${context}` : "Internal Error"
  notifyError(title, error)
}

export function logError(title: string, error: unknown, description: string | null = null): void {
  console.error(title, error)
  // noinspection JSIgnoredPromiseFromCall
  logErrorToBackend(title, { keyValues: { error: description || getErrorDetails(error) } })
}
