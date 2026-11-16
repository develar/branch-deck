export const isMac = navigator.userAgent.toLowerCase().includes("mac")

/** Windows: the taskbar has no badge count (`setBadgeCount` is unsupported there) */
export const isWindows = navigator.userAgent.toLowerCase().includes("windows")

/** Linux (and the other X11/Wayland desktops): neither macOS nor Windows */
export const isLinux = !isMac && !isWindows
