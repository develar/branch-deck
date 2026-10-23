import { commands } from "@/lib/bindings"
import { getErrorCode } from "@/lib/errors"
import type { ToastMessage } from "@/lib/toast-message"

/**
 * Stop Waiting: ends the wait of the command whose channel is `channel` (the `wait` of a rewrite, the progress of a push
 * or an update) for another holder of the repository lock. The command then answers the code `canceled` and changed
 * nothing; one whose wait ended already (it holds the lock) goes on
 */
export function stopLockWait(channel: { readonly id: number }): void {
  void commands.cancelLockWait(channel.id).catch(() => {})
}

/** Whether a command failed because the user stopped its wait for the repository lock (`stopLockWait`): nothing changed */
export function isLockWaitCanceled(error: unknown): boolean {
  return getErrorCode(error) === "canceled"
}

/**
 * The toast of an action whose wait for the repository lock the user stopped: "Canceled the push of auth before it
 * started", "Canceled squashing 2 commits before it started"; `action` names it in the middle of a sentence
 */
export function canceledBeforeStart(action: string, nothing = "Nothing changed"): ToastMessage {
  return { type: "info", title: `Canceled ${action} before it started`, description: nothing }
}

/** The busy state of an operation ("Squashing 2 commits") as the action in the middle of a sentence */
export function actionOfLabel(label: string): string {
  return label.charAt(0).toLowerCase() + label.slice(1)
}
