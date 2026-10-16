import { useEffect } from "react"
import { useDockStatus } from "@/hooks/useDockStatus"
import { useRunProgress } from "@/hooks/useRunProgress"
import { setDockProgress } from "@/lib/dock"
import { progressPercent } from "@/lib/update-progress"
import { isMainWindow } from "@/stores/settings/app-store"
import type { RunningUpdate } from "@/stores/update-project"
import { useShownUpdate } from "@/stores/update-project"

/**
 * The Dock progress and badge of the main window (`useDockStatus`); renders nothing, so its tick renders only itself.
 * While an update or a fetch runs in the selected repository, the Dock shows its progress instead of the robots': the
 * user started it and waits for it, maybe in another app.
 */
export function DockStatus() {
  const shown = useShownUpdate()
  const running = shown?.endedAt === null ? shown : undefined
  useDockStatus(running !== undefined)
  return running === undefined ? null : <UpdateDockProgress shown={running} />
}

/** The progress of the running update in the Dock, set when its whole percent changes */
function UpdateDockProgress({ shown }: { shown: RunningUpdate }) {
  const percent = progressPercent(useRunProgress(shown))
  useEffect(() => {
    if (isMainWindow) {
      void setDockProgress({ status: "normal", progress: percent })
    }
  }, [percent])
  return null
}
