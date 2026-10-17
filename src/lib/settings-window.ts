import { runAppCommand } from "@/lib/app-commands"
import { SETTINGS_PAGES } from "@/lib/settings-pages"
import type { SettingsPageId } from "@/lib/settings-pages"

/*
 * The Settings window (spec/repository/settings.spec.md): a sub-window of its own, opened by the main window only, which
 * has the current settings to start it from. Every entry point asks the main window (`openSettings`).
 */

export const SETTINGS_WINDOW = "settings"

/** Phase C2 builds the Repository page; until then its tab is not shown and nothing opens it */
export const REPOSITORY_PAGE_READY = false

/** What an open of the window shows (`__INIT_DATA__`): a page, a field of it to focus; none keeps the current page */
export interface SettingsTarget {
  readonly page?: SettingsPageId
  /** The `data-settings-focus` of the field */
  readonly focus?: string
}

/** The pages the sidebar shows */
export function shownSettingsPages(): SettingsPageId[] {
  return SETTINGS_PAGES.filter(page => page !== "repository" || REPOSITORY_PAGE_READY)
}

/** The page an open shows: the one it names, else `current`; a page the sidebar doesn't show is General */
export function resolveSettingsPage(current: SettingsPageId, target: SettingsTarget | null): SettingsPageId {
  const page = target?.page ?? current
  return shownSettingsPages().includes(page) ? page : "general"
}

/** Opens Settings at `target`, from any window: the main window opens it (`useAppCommand("open_settings")` of `IndexPage`) */
export function openSettings(target: SettingsTarget = {}): void {
  runAppCommand("open_settings", target)
}
