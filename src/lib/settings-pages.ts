/** The pages of the sidebar of Settings, in its order (`src/lib/settings-window.ts`); the last one shown is saved */
export const SETTINGS_PAGES = ["general", "appearance", "notifications", "github", "repository"] as const

export type SettingsPageId = (typeof SETTINGS_PAGES)[number]
