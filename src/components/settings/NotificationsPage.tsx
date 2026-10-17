import { useEffect, useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { BellIcon, ExternalLinkIcon, SendIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { Callout } from "@/components/shared/Callout"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { notifyError } from "@/lib/errors"
import type { NotificationPermissionState } from "@/lib/os-notification"
import { checkNotificationPermission, requestNotificationPermission, sendTestNotification } from "@/lib/os-notification"
import { isMac, isWindows } from "@/lib/platform"

/** The notification settings of the system, where the user allows what they refused */
const systemNotificationSettingsUrl = isMac ? "x-apple.systempreferences:com.apple.Notifications-Settings.extension" : isWindows ? "ms-settings:notifications" : null

/**
 * Settings → Notifications: the OS notifications in the background, whether the OS lets the app notify at all (asked
 * again on each open of the window, `openCount`, and on focus), what the badge of the Dock counts
 */
export function NotificationsPage({ openCount }: { openCount: number }) {
  const [permission, setPermission] = useState<NotificationPermissionState | null>(null)
  useEffect(() => {
    void checkNotificationPermission().then(setPermission)
  }, [openCount])
  useWindowFocus(() => {
    void checkNotificationPermission().then(setPermission)
  })
  return (
    <>
      <SettingsSection title="Notifications" description="When Branch Deck is in the background; in front, a toast tells it.">
        <SettingSwitch name="notifyReviewRequests" label="Review requests" description="A pull request asks for your review" />
        <SettingSwitch name="notifyPatronusMerges" label="Safe Push merges" description="A robot merged one of your branches" />
        <SettingSwitch name="notifyPatronusFailures" label="Safe Push failures" description="A required check failed, or the robot failed" />
        <SettingSwitch name="notifyApprovals" label="Approvals of my pull requests" description="A code owner approved one of your pull requests" />
        <NotificationPermission permission={permission} onChange={setPermission} />
      </SettingsSection>
      {!isWindows && (
        <SettingsSection title="Dock icon" description="The badge on the app icon counts">
          <SettingSwitch name="reviewBadge" label="Count review requests" />
          <SettingSwitch name="attentionBadge" label="Count failed robots and conflicts" />
        </SettingsSection>
      )}
    </>
  )
}

/** Not asked: "Allow Notifications"; refused: where to allow them; granted: a test notification */
function NotificationPermission({ permission, onChange }: { permission: NotificationPermissionState | null; onChange: (permission: NotificationPermissionState) => void }) {
  const [sent, setSent] = useState(false)
  switch (permission) {
    case null:
      return null
    case "notAsked":
      return (
        <div className="flex" data-testid="notification-permission" data-status="notAsked">
          <Button size="xs" variant="outline" onClick={() => void requestNotificationPermission().then(onChange)}>
            <BellIcon data-icon="inline-start" />
            Allow Notifications
          </Button>
        </div>
      )
    case "denied":
      // one line with its button: the page fits the window with the Dock badge below
      return (
        <Callout
          tone="warning"
          title={
            <span className="flex flex-wrap items-center justify-between gap-x-2">
              Notifications are off in System Settings
              {systemNotificationSettingsUrl !== null && (
                <Button
                  size="xs"
                  variant="ghost"
                  className="-my-1 text-muted-foreground"
                  onClick={() => void openUrl(systemNotificationSettingsUrl).catch((error: unknown) => notifyError("Cannot open System Settings", error))}
                >
                  <ExternalLinkIcon data-icon="inline-start" />
                  Open System Settings
                </Button>
              )}
            </span>
          }
          data-testid="notification-permission"
          data-status="denied"
        />
      )
    case "granted":
      return (
        <div className="flex items-center gap-3" data-testid="notification-permission" data-status="granted">
          <Button
            size="xs"
            variant="outline"
            onClick={() => {
              sendTestNotification()
              setSent(true)
            }}
          >
            <SendIcon data-icon="inline-start" />
            Send Test Notification
          </Button>
          {sent && (
            <span className="text-xs text-muted-foreground" data-testid="test-notification-sent">
              Sent
            </span>
          )}
        </div>
      )
  }
}
