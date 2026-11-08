/**
 * The pages of the sidebar of Settings, in its order (`src/lib/settings-window.ts`): the pages of the app, then those of
 * the git config of the repositories, which show the scope tabs; the last one shown is saved
 */
export const SETTINGS_PAGES = ["general", "appearance", "diffs", "notifications", "accounts", "tools", "branches", "landing", "agents"] as const

export type SettingsPageId = (typeof SETTINGS_PAGES)[number]

/** The pages of the `branchdeck.*` git config, under the scope tabs "All repositories | <repository>" */
export const REPOSITORY_PAGES: readonly SettingsPageId[] = ["branches", "landing", "agents"]

/** The pages of the builds before the scope tabs, by the page that has their settings now */
const RETIRED_PAGES: Readonly<Record<string, SettingsPageId>> = {
  github: "accounts",
  youtrack: "accounts",
  repository: "branches",
  companions: "landing",
}

/** A page of the sidebar for `value`: a page of before is the page that has its settings now, anything else as it is */
export function currentSettingsPage(value: unknown): unknown {
  return typeof value === "string" ? (RETIRED_PAGES[value] ?? value) : value
}
