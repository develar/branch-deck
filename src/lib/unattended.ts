import { MoonIcon, MoonStarIcon } from "lucide-react"
import { installAgentCli } from "@/hooks/useInstallAgentCli"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import { showToast } from "@/lib/toast-message"
import { offersDeniedRequests, offersInstall, SHOW_DENIED_REQUESTS, UNATTENDED_DESCRIPTION, unattendedModeLabel, unattendedToast } from "@/presenters/unattended"
import { openDeniedRequests } from "@/stores/unattended"

/**
 * Turns Unattended Mode on or off (`set_unattended_mode`, openspec/specs/repository/unattended-mode/spec.md); the title
 * bar hears the setting from the writer as every window does. Its toast: a switch on without the hook offers Install
 * Agent CLI, a switch off tells the requests the night denied and opens their list
 */
export async function setUnattendedMode(on: boolean, listHere = true): Promise<void> {
  const result = await commands.setUnattendedMode(on)
  if (result.status === "error") {
    notifyError(on ? "Cannot turn on Unattended Mode" : "Cannot turn off Unattended Mode", result.error)
    return
  }
  const mode = result.data
  const toast = unattendedToast(mode)
  if (toast === null) {
    return
  }
  if (offersInstall(mode)) {
    showToast({ ...toast, action: { children: "Install Agent CLI", onClick: () => void installAgentCli(false) } })
  } else if (offersDeniedRequests(mode, listHere)) {
    showToast({ ...toast, action: { children: SHOW_DENIED_REQUESTS, onClick: () => openDeniedRequests(mode.denials) } })
  } else {
    showToast(toast)
  }
}

/**
 * The one item of the mode, which the mark of the title bar, the command palette and Settings → General show: its switch
 * by the mode as it is; `listHere` false in a window without the list of Denied Requests (Settings)
 */
export function unattendedModeItem(on: boolean, listHere = true): MenuItem {
  return {
    id: "unattended_mode",
    label: unattendedModeLabel(on),
    icon: on ? MoonStarIcon : MoonIcon,
    description: UNATTENDED_DESCRIPTION,
    onSelect: () => void setUnattendedMode(!on, listHere),
  }
}
