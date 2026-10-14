import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { toast } from "@/components/ui/toast"

/** The message of an error for the user: the `message` of an `Error` (no "Error:" prefix) or of a `CommandError` */
export function getErrorDetails(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  // CommandError returned by Tauri commands
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    return error.message
  }
  return String(error)
}

/** Logs the error and shows a sticky error toast. Usable outside React. */
export function notifyError(title: string, error: unknown): void {
  const description = getErrorDetails(error)
  logError(title, error, description)

  toast.add({
    type: "error",
    title,
    description,
    // don't auto-dismiss errors
    timeout: 0,
  })
}

export function notifyInternalError(error: unknown, context?: string): void {
  const title = context ? `Internal Error: ${context}` : "Internal Error"
  notifyError(title, error)
}

export function logError(title: string, error: unknown, description: string | null = null): void {
  console.error(title, error)
  logErrorToBackend(title, { keyValues: { error: description ?? getErrorDetails(error) } })
    .catch((logFailure: unknown) => {
      console.error("Failed to forward error to backend log", logFailure)
    })
}
