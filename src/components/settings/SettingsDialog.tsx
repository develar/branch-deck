import { useEffect, useId, useRef, useState } from "react"
import type { ReactNode } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { BellIcon, ExternalLinkIcon, SendIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Switch } from "@/components/ui/switch"
import { BranchPrefixSetting } from "@/components/settings/BranchPrefixSetting"
import { GitHubSettings } from "@/components/settings/GitHubSettings"
import { Callout } from "@/components/shared/Callout"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { changeAIMode } from "@/hooks/useAIToggle"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { notifyError } from "@/lib/errors"
import type { NotificationPermissionState } from "@/lib/os-notification"
import { checkNotificationPermission, requestNotificationPermission, sendTestNotification } from "@/lib/os-notification"
import { isMac, isWindows } from "@/lib/platform"
import { refreshGitHubAuthStatus } from "@/stores/github-auth"
import { aiConsentNote, useAISettings } from "@/stores/settings/aiSettings"
import type { AppSettingsSwitch } from "@/stores/settings/appSettings"
import { setAppSettingsSwitch, useAppSettings } from "@/stores/settings/appSettings"

interface SettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** The notification settings of the system, where the user allows what they refused */
const systemNotificationSettingsUrl = isMac ? "x-apple.systempreferences:com.apple.Notifications-Settings.extension" : isWindows ? "ms-settings:notifications" : null

/**
 * "Settings…" (⌘, in the app menu on macOS, File → Settings… elsewhere, and the menu of the Your Turn card): the branch
 * prefix of every repository; the OS
 * notifications in the background and what the badge of the Dock counts, synced across windows; whether the OS
 * lets the app notify at all, asked again on open and on focus; whether branch names are suggested by Claude; and the
 * GitHub token, with the app's own sign-in.
 */
export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const bodyRef = useRef<HTMLDivElement>(null)
  const [permission, setPermission] = useState<NotificationPermissionState | null>(null)
  useEffect(() => {
    if (open) {
      void checkNotificationPermission().then(setPermission)
      void refreshGitHubAuthStatus()
    }
  }, [open])
  useWindowFocus(() => {
    if (open) {
      void checkNotificationPermission().then(setPermission)
    }
  })

  return (
    <PreviewDialogFrame data-testid="settings-dialog" width="md" open={open} onOpenChange={onOpenChange} initialFocus={bodyRef}>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Settings</DialogTitle>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef}>
        <div className="flex flex-col gap-6">
          <SettingsSection title="Branches" description="The prefix of your virtual branches, in every repository">
            <BranchPrefixSetting />
          </SettingsSection>
          <SettingsSection title="Notifications" description="When Branch Deck is in the background; in front, a toast tells it.">
            <SettingSwitch name="notifyReviewRequests" label="Review requests" description="A pull request asks for your review" />
            <SettingSwitch name="notifyPatronusMerges" label="Safe Push merges" description="A robot merged one of your branches" />
            <SettingSwitch name="notifyPatronusFailures" label="Safe Push failures" description="A required check failed, or the robot failed" />
            <NotificationPermission permission={permission} onChange={setPermission} />
          </SettingsSection>
          {!isWindows && (
            <SettingsSection title="Dock icon" description="The badge on the app icon counts">
              <SettingSwitch name="reviewBadge" label="Count review requests" />
              <SettingSwitch name="attentionBadge" label="Count failed robots and conflicts" />
            </SettingsSection>
          )}
          <SettingsSection title="GitHub" description="Pull requests and the requests for your review">
            <GitHubSettings />
          </SettingsSection>
          <SettingsSection title="AI" description="Branch names for the selected commits in Move to…">
            <AISwitch />
          </SettingsSection>
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}

function SettingsSection({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  const titleId = useId()
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-3">
      <div className="flex flex-col gap-0.5">
        <h3 id={titleId} className="text-sm font-medium">
          {title}
        </h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      {children}
    </section>
  )
}

function SettingSwitch({ name, label, description }: { name: AppSettingsSwitch; label: string; description?: string }) {
  const checked = useAppSettings(state => state[name])
  return <SettingRow testId={`setting-${name}`} label={label} description={description} checked={checked} onCheckedChange={value => setAppSettingsSwitch(name, value)} />
}

/** On is the consent of the prompt in Move to…; off is its "Not now", and so is no answer yet */
function AISwitch() {
  const checked = useAISettings(state => state.aiMode === "claude")
  return (
    <SettingRow
      testId="setting-aiMode"
      label="Suggest branch names"
      description={aiConsentNote}
      checked={checked}
      onCheckedChange={value => changeAIMode(value ? "claude" : "disabled")}
    />
  )
}

function SettingRow({
  testId,
  label,
  description,
  checked,
  onCheckedChange,
}: {
  testId: string
  label: string
  description?: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <label className="flex items-start gap-3 text-sm" data-testid={testId}>
      <Switch size="sm" className="mt-0.5" checked={checked} onCheckedChange={onCheckedChange} />
      <span className="flex flex-col gap-0.5">
        {label}
        {description !== undefined && <span className="text-xs text-muted-foreground">{description}</span>}
      </span>
    </label>
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
      return (
        <Callout tone="warning" title="Notifications are off for Branch Deck in System Settings" data-testid="notification-permission" data-status="denied">
          {systemNotificationSettingsUrl !== null && (
            <div className="-ml-2 flex">
              <Button
                size="xs"
                variant="ghost"
                onClick={() => void openUrl(systemNotificationSettingsUrl).catch((error: unknown) => notifyError("Cannot open System Settings", error))}
              >
                <ExternalLinkIcon data-icon="inline-start" />
                Open System Settings
              </Button>
            </div>
          )}
        </Callout>
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
