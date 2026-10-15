export const isMac = navigator.userAgent.toLowerCase().includes("mac")

/** Windows: the taskbar has no badge count (`setBadgeCount` is unsupported there) */
export const isWindows = navigator.userAgent.toLowerCase().includes("windows")

/** Label of the primary shortcut modifier: Cmd on macOS, Ctrl elsewhere */
export const modifierKey = isMac ? "⌘" : "Ctrl"
