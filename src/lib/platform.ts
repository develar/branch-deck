export const isMac = navigator.userAgent.toLowerCase().includes("mac")

/** Label of the primary shortcut modifier: Cmd on macOS, Ctrl elsewhere */
export const modifierKey = isMac ? "⌘" : "Ctrl"
