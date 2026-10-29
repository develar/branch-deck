import { toast } from "@/components/ui/toast"

/** The action of a toast: a toast about an object opens it ("Open in Patronus", "Open Pull Request") */
export interface ToastActionProps {
  children: string
  onClick: () => void
}

/** A toast of what happened, in the words of docs/contributing/ux-writing.md; an error goes through `notifyError` */
export interface ToastMessage {
  /** `info` for a precondition, nothing to do ("No commits to unapply"); `warning` for a part that failed */
  type?: "success" | "info" | "warning"
  title: string
  description?: string
  action?: ToastActionProps
}

/** A toast with an action stays 10 s, as the Undo of a rewrite, so there is time to act */
export const ACTION_TOAST_TIMEOUT_MS = 10_000

/** A toast without an action stays 5 s, the timeout of the `Toaster` */
export const TOAST_TIMEOUT_MS = 5_000

/** Shows the toast of what happened */
export function showToast({ type = "success", title, description, action }: ToastMessage): void {
  toast.add({ type, title, description, ...(action !== undefined && { timeout: ACTION_TOAST_TIMEOUT_MS, actionProps: action }) })
}
