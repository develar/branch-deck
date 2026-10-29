/**
 * The theme of every window (Settings → Appearance → Theme): the system's, Light or Dark. The page shows the tokens of
 * `src/index.css` by `data-theme` on the root (docs/design/ui-theme.md, "Dark mode"), the window its frame by the
 * theme of Tauri
 */
export const THEMES = ["system", "light", "dark"] as const

export type Theme = (typeof THEMES)[number]

export type ShownTheme = "light" | "dark"

/** What a window shows of the setting, the system's dark appearance given */
export function shownTheme(theme: Theme, systemDark: boolean): ShownTheme {
  if (theme === "system") {
    return systemDark ? "dark" : "light"
  }
  return theme
}

/** The tokens of the theme on the root of the page, and the colour scheme of its scroll bars and form controls */
export function showTheme(root: HTMLElement, theme: ShownTheme): void {
  root.dataset.theme = theme
  root.style.colorScheme = theme
}
