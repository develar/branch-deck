import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

/**
 * Restarts Branch Deck with "Use the system title bar" at `systemTitleBar`, which the backend saves before the restart
 * (`restart_app`): the restart of a setting of Appearance and Restart to Update of a replaced binary. A command of a
 * Result never rejects
 */
export function restartBranchDeck(systemTitleBar: boolean): void {
  void commands.restartApp(systemTitleBar).then(result => {
    if (result.status === "error") {
      notifyError("Cannot restart Branch Deck", result.error)
    }
  })
}
