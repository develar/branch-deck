import { openUrl } from "@tauri-apps/plugin-opener"
import { BookOpenIcon, CircleArrowUpIcon, CommandIcon, GitBranchIcon, HistoryIcon, RefreshCwIcon, SettingsIcon, TerminalIcon, ZoomInIcon, ZoomOutIcon } from "lucide-react"
import { openActivity } from "@/lib/activity-window"
import { runAppCommand } from "@/lib/app-commands"
import type { AppCommand } from "@/lib/app-commands"
import { notifyError } from "@/lib/errors"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { openSettings } from "@/lib/settings-window"
import { setAutoSyncOnFocus } from "@/stores/settings/appSettings"

/*
 * The main menu of a custom title bar on Windows and Linux (☰, `MainMenuButton`), which replaces the native menu bar
 * there: the items of the native menu (`src-tauri/src/menu.rs`), one builder for its surfaces, and the keys the native
 * accelerators had (`matchMainMenuKey`, `useMainMenuKeys`). Each item runs the command the native item runs
 * (`runAppCommand`).
 */

export type MainMenuId = "file" | "view" | "help"

export interface MainMenu {
  readonly id: MainMenuId
  readonly label: string
  /** The letter of Alt+letter, underlined while the menu is opened from the keyboard */
  readonly mnemonic: string
  readonly groups: MenuItemGroups
}

export interface MainMenuState {
  /** Whether Sync can run, and why not (`useSyncAvailability`) */
  readonly sync: { readonly canSync: boolean; readonly reason: string | null }
  readonly autoSyncOnFocus: boolean
  /** The build updates itself (the backend says it, `titleBar.checkForUpdates`) */
  readonly updates: boolean
}

const REPOSITORY_URL = "https://github.com/develar/branch-deck"

function open(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the link", error))
}

export function mainMenus(state: MainMenuState): MainMenu[] {
  return [
    {
      id: "file",
      label: "File",
      mnemonic: "F",
      groups: [
        [
          {
            id: "sync_branches",
            label: "Sync Branches",
            icon: RefreshCwIcon,
            kbds: ["mod", "r"],
            disabled: !state.sync.canSync,
            ...(state.sync.reason !== null && { description: state.sync.reason }),
            onSelect: () => runAppCommand("sync_branches"),
          },
          { id: "update_project", label: "Update Project", icon: GitBranchIcon, kbds: ["mod", "t"], onSelect: () => runAppCommand("update_project") },
        ],
        [{ id: "auto_sync_on_focus", label: "Auto-sync on Focus", checked: state.autoSyncOnFocus, onSelect: () => setAutoSyncOnFocus(!state.autoSyncOnFocus) }],
        [{ id: "open_settings", label: "Settings…", icon: SettingsIcon, kbds: ["mod", ","], onSelect: () => openSettings() }],
      ],
    },
    {
      id: "view",
      label: "View",
      mnemonic: "V",
      groups: [
        [{ id: "open_command_palette", label: "Command Palette…", icon: CommandIcon, kbds: ["mod", "k"], onSelect: () => runAppCommand("open_command_palette") }],
        zoomMenuItems(),
        [{ id: "open_activity", label: "Activity", icon: HistoryIcon, onSelect: () => openActivity() }],
      ],
    },
    {
      id: "help",
      label: "Help",
      mnemonic: "H",
      groups: [
        [
          { id: "getting_started", label: "Getting Started", icon: BookOpenIcon, onSelect: () => open(`${REPOSITORY_URL}#getting-started`) },
          { id: "install_agent_cli", label: "Install Agent CLI…", icon: TerminalIcon, onSelect: () => runAppCommand("install_agent_cli") },
          ...(state.updates ? [{ id: "check_for_updates", label: "Check for Updates…", icon: CircleArrowUpIcon, onSelect: () => runAppCommand("check_for_updates") }] : []),
        ],
        [{ id: "github_link", label: "GitHub Repository", onSelect: () => open(REPOSITORY_URL) }],
      ],
    },
  ]
}

/**
 * The zoom items of the View menu, in Safari's order: the menu and the zoom of Settings → Appearance (`ZoomStepper`)
 * pick them by id
 */
export function zoomMenuItems(): MenuItem[] {
  return [
    { id: "zoom_reset", label: "Actual Size", kbds: ["mod", "0"], onSelect: () => runAppCommand("zoom", "reset") },
    { id: "zoom_in", label: "Zoom In", icon: ZoomInIcon, kbds: ["mod", "="], onSelect: () => runAppCommand("zoom", "in") },
    { id: "zoom_out", label: "Zoom Out", icon: ZoomOutIcon, kbds: ["mod", "-"], onSelect: () => runAppCommand("zoom", "out") },
  ]
}

/**
 * The command of a key the native menu's accelerator had: Ctrl+R, Ctrl+T, Ctrl+,, Ctrl+K and the zoom keys (Ctrl+0,
 * Ctrl+=, Ctrl+-). The physical key, as the native accelerators go by the virtual key: a Cyrillic layout's Ctrl+R still syncs
 * (and WebView2 doesn't reload). Ctrl+Shift+= is `useZoom`'s other Zoom In key.
 */
export function matchMainMenuKey(event: Pick<KeyboardEvent, "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): { command: AppCommand; payload?: string } | null {
  if (!event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) {
    return null
  }
  switch (event.code) {
    case "KeyR":
      return { command: "sync_branches" }
    case "KeyT":
      return { command: "update_project" }
    case "Comma":
      return { command: "open_settings" }
    case "KeyK":
      return { command: "open_command_palette" }
    case "Digit0":
    case "Numpad0":
      return { command: "zoom", payload: "reset" }
    case "Equal":
      return { command: "zoom", payload: "in" }
    case "Minus":
    case "NumpadSubtract":
      return { command: "zoom", payload: "out" }
    default:
      return null
  }
}

/** The menu Alt+letter, Alt+\ (File, IntelliJ's `MainMenuButton.ShowMenu`) or F10 opens, by the physical key */
export function matchMainMenuOpenKey(event: Pick<KeyboardEvent, "code" | "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): MainMenuId | null {
  if (event.ctrlKey || event.metaKey || event.shiftKey) {
    return null
  }
  if (!event.altKey) {
    return event.key === "F10" ? "file" : null
  }
  switch (event.code) {
    case "Backslash":
    case "KeyF":
      return "file"
    case "KeyV":
      return "view"
    case "KeyH":
      return "help"
    default:
      return null
  }
}
