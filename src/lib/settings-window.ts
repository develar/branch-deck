import { runAppCommand } from "@/lib/app-commands"
import { currentSettingsPage, SETTINGS_PAGES } from "@/lib/settings-pages"
import type { SettingsPageId } from "@/lib/settings-pages"

/*
 * The Settings window (openspec/specs/repository/settings/spec.md): a sub-window of its own, opened by the main window only, which
 * has the current settings to start it from. Every entry point asks the main window (`openSettings`).
 */

export const SETTINGS_WINDOW = "settings"

/**
 * The size Settings opens with, every time: the window-state plugin does not restore it (`src-tauri/src/lib.rs`). Every
 * page fits it in its usual state; the minimum is `minSubWindowSize` of `src/lib/zoom.ts`
 */
export const SETTINGS_WINDOW_SIZE = { width: 880, height: 640 } as const

/** The scope tabs of the pages of the git config: the value of every repository, or the selected repository's own */
export type SettingsScope = "all" | "repository"

/** What an open of the window shows (`__INIT_DATA__`): a page, its scope, a field of it to focus; none keeps the current page */
export interface SettingsTarget {
  readonly page?: SettingsPageId
  /** The scope tab of a page of the git config; "All repositories" when none */
  readonly scope?: SettingsScope
  /** The `data-settings-focus` of the field */
  readonly focus?: string
  /** Settings → Tools: the TeamCity server to sign `teamcity` in to (the server of a robot's build) */
  readonly server?: string
}

/**
 * The page an open shows: the one it names, else `current`; a page of an older build is the page that has its settings
 * now, one the sidebar doesn't know is General
 */
export function resolveSettingsPage(current: SettingsPageId, target: SettingsTarget | null): SettingsPageId {
  const page = currentSettingsPage(target?.page ?? current)
  return SETTINGS_PAGES.find(it => it === page) ?? "general"
}

/**
 * The scope tab an open shows: the one it names, else "All repositories", never remembered from an open before (a
 * change there would go to the repository's own config unseen). Without a repository the page shows "All
 * repositories" whatever it names (`SettingsPage`)
 */
export function resolveSettingsScope(target: SettingsTarget | null): SettingsScope {
  return target?.scope === "repository" ? "repository" : "all"
}

/** Opens Settings at `target`, from any window: the main window opens it (`useAppCommand("open_settings")` of `IndexPage`) */
export function openSettings(target: SettingsTarget = {}): void {
  runAppCommand("open_settings", target)
}
