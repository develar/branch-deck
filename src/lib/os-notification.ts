import { getAllWindows, getCurrentWindow } from "@tauri-apps/api/window"
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification"
import type { AppSettingsSwitch } from "@/stores/settings/appSettings"
import { appSettingsStore } from "@/stores/settings/appSettings"

/*
 * The notifications of the main window (openspec/specs/repository/review-requests/spec.md#settings): news is a toast while a
 * Branch Deck window has the focus; in the background an OS notification says it at once, if its switch in Settings is
 * on, and the toast waits for the focus. An OS notification has no click action.
 */

/** What an OS notification is about: each has its switch in Settings */
export type NotificationKind = "reviewRequests" | "patronusMerges" | "patronusFailures" | "approvals"

const kindSwitches: Readonly<Record<NotificationKind, AppSettingsSwitch>> = {
  reviewRequests: "notifyReviewRequests",
  patronusMerges: "notifyPatronusMerges",
  patronusFailures: "notifyPatronusFailures",
  approvals: "notifyApprovals",
}

/**
 * What an OS notification says; `news` names the news of a session it tells, which the host of the page in the IDE
 * (`brd mcp`) knows: it gives the notification the action that opens its object, or leaves it out when it told the
 * news itself (openspec/specs/repository/ide-embedding/spec.md)
 */
export interface OsNotification {
  readonly title: string
  readonly body: string
  readonly news?: { readonly repositoryPath: string; readonly id: number }
}

/** The permission of the app to show OS notifications: never asked, refused in the system settings, or granted */
export type NotificationPermissionState = "notAsked" | "denied" | "granted"

let windowFocused = typeof document === "undefined" || document.hasFocus()
/** null until a notification or Settings asked the OS */
let permissionGranted: boolean | null = null
/** The app asked the OS for the permission once; it asks again only on "Allow Notifications" */
let permissionRequested = false
/** The toasts of news told while no window had the focus: shown on the next focus of the main window */
let deferredToasts: (() => void)[] = []

/** Whether the main window has the focus */
export function isWindowFocused(): boolean {
  return windowFocused
}

/**
 * The main window gained or lost the focus. On focus the deferred toasts show, and a refused permission is checked
 * again: the user may have allowed the notifications in the system settings meanwhile.
 */
export function setWindowFocus(focused: boolean): void {
  windowFocused = focused
  if (!focused) {
    return
  }
  const toasts = deferredToasts
  deferredToasts = []
  for (const show of toasts) {
    show()
  }
  if (permissionGranted === false) {
    void checkNotificationPermission()
  }
}

/** Whether a Branch Deck window has the focus: the main one, or a sub-window (conflicting files) beside it */
async function isAppFocused(): Promise<boolean> {
  if (windowFocused) {
    return true
  }
  try {
    const current = getCurrentWindow().label
    for (const window of await getAllWindows()) {
      if (window.label !== current && (await window.isFocused())) {
        return true
      }
    }
  } catch {
    // no window API (unit tests): the main window decides
  }
  return false
}

/**
 * Tells news: `showToast` at once while a Branch Deck window has the focus; else the OS notification of `kind` (when
 * Settings lets it) now and `showToast` on the next focus of the main window
 */
export function announce(kind: NotificationKind, notification: OsNotification, showToast: () => void): void {
  if (windowFocused) {
    showToast()
    return
  }
  void isAppFocused().then(focused => {
    // the focus may have come back meanwhile
    if (focused || windowFocused) {
      showToast()
      return
    }
    deferredToasts.push(showToast)
    void notifyOs(kind, notification)
  })
}

function isKindEnabled(kind: NotificationKind): boolean {
  return appSettingsStore.getState()[kindSwitches[kind]]
}

/**
 * Sends an OS notification of `kind` unless Settings turned it off. Asks for the permission on the first notification,
 * never at startup; a notification is an extra, it never fails.
 */
async function notifyOs(kind: NotificationKind, { title, body, news }: OsNotification): Promise<void> {
  if (!isKindEnabled(kind)) {
    return
  }
  try {
    if (permissionGranted !== true) {
      permissionGranted = await isPermissionGranted()
      if (!permissionGranted && !permissionRequested) {
        permissionRequested = true
        permissionGranted = (await requestPermission()) === "granted"
      }
    }
    if (permissionGranted) {
      sendNotification({ title, body, ...(news !== undefined && { extra: { repositoryPath: news.repositoryPath, news: news.id } }) })
    }
  } catch {
    // no notification plugin (the E2E mock), or the OS refused
  }
}

/** Asks the OS whether the app may notify, for Settings and after a refusal; "notAsked" when the OS can't say */
export async function checkNotificationPermission(): Promise<NotificationPermissionState> {
  try {
    permissionGranted = await isPermissionGranted()
    if (permissionGranted) {
      return "granted"
    }
    return typeof window !== "undefined" && window.Notification?.permission === "denied" ? "denied" : "notAsked"
  } catch {
    return "notAsked"
  }
}

/** "Allow Notifications" of Settings: asks the OS, which may show its prompt */
export async function requestNotificationPermission(): Promise<NotificationPermissionState> {
  try {
    permissionRequested = true
    const answer = await requestPermission()
    permissionGranted = answer === "granted"
    return permissionGranted ? "granted" : answer === "denied" ? "denied" : "notAsked"
  } catch {
    return "notAsked"
  }
}

/** "Send Test Notification" of Settings, whatever the switches say */
export function sendTestNotification(): void {
  try {
    sendNotification({ title: "Branch Deck", body: "Notifications work: a review request shows like this." })
  } catch {
    // the OS refused: Settings shows the permission
  }
}

/** Forgets the focus, the permission and the deferred toasts (tests) */
export function resetOsNotification(): void {
  windowFocused = true
  permissionGranted = null
  permissionRequested = false
  deferredToasts = []
}
