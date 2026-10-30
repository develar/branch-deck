import { installUpdateNow } from "@/hooks/useAutoUpdate"
import { runAppCommand } from "@/lib/app-commands"
import type { SetupEntry } from "@/lib/bindings"
import { openSettings } from "@/lib/settings-window"
import { setupSkipUntil } from "@/lib/setup"
import { runCliToolAction } from "@/stores/cli-tools"
import { skipSetupItem } from "@/stores/settings/appSettings"
import { refreshSetup } from "@/stores/setup"

/** The action of an item of the Setup dot, as its row and Settings → Tools run it */
export function runSetupItem(entry: SetupEntry): void {
  const item = entry.item
  switch (item.kind) {
    case "appUpdate":
      void installUpdateNow()
      return
    case "appUpdateCheckFailed":
      runAppCommand("check_for_updates")
      return
    case "toolUpdate":
      void runCliToolAction(item.tool, "update", item.version).then(refreshSetup)
      return
    case "skillUpdate":
      void runCliToolAction(item.tool, "updateSkill", null).then(refreshSetup)
      return
    case "signInExpiring":
    case "signInExpired":
      openSettings({ page: "tools", focus: item.tool })
      return
    case "youTrackSignIn":
      openSettings({ page: "youtrack" })
  }
}

/** Skip This Version or Remind Tomorrow: every window hides the item (`appSettings.setupSkips`) */
export function skipSetupEntry(entry: SetupEntry): void {
  skipSetupItem(entry.key, setupSkipUntil(entry, Math.floor(Date.now() / 1000)))
}
