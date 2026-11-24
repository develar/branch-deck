import { useTauriListen } from "@/hooks/useTauriListen"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { restartBranchDeck } from "@/lib/restart"
import { appSettingsStore } from "@/stores/settings/appSettings"

/** The event of a binary of the app replaced on disk while it runs (`app_api::app::life::BUILD_REPLACED_EVENT`) */
const BUILD_REPLACED_EVENT = "build-replaced"

/**
 * A newer Branch Deck replaced the binary of the app while its window is open, and the IDE asked for it
 * (openspec/specs/repository/backend-protocol/spec.md, "Socket: The stale side yields to a client of another build"):
 * Rust decides it and tells the main window once, which words it with Restart to Update. Main window only.
 */
export function useBuildReplaced(): void {
  useTauriListen<null>(BUILD_REPLACED_EVENT, () => {
    showEventToast({
      key: toastKeys.buildReplaced(),
      render: () => ({
        type: "info",
        title: "A newer Branch Deck is installed",
        description: "Restart to run it here and in the IDE",
        action: { label: "Restart to Update", onClick: () => restartBranchDeck(appSettingsStore.getState().systemTitleBar) },
        timeout: 0,
      }),
    })
  })
}
