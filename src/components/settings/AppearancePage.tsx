import { RotateCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { SettingRow, SettingsSection } from "@/components/settings/SettingRow"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { zoomMenuItems } from "@/lib/main-menu"
import { kbdLabel } from "@/lib/menu"
import type { MenuItem } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { titleBar } from "@/lib/title-bar"
import type { NativeTitleBarReason } from "@/lib/title-bar"
import { setSystemTitleBar, useAppSettings } from "@/stores/settings/appSettings"
import { setDiffSoftWraps, useDiffSoftWraps } from "@/stores/settings/diff-view-settings"

/** Settings → Appearance: the zoom of every window, the diffs; off macOS, the system title bar */
export function AppearancePage() {
  return (
    <>
      <SettingsSection title="Zoom" description="The size of the text and the controls in every window, also in the View menu">
        <ZoomStepper />
      </SettingsSection>
      <SettingsSection title="Diffs" description="Every diff of the app: the commit preview, the reviews, the conflicts">
        <DiffSettings />
      </SettingsSection>
      {!isMac && (
        <SettingsSection title="Title bar" description="The title bar of the windows">
          <SystemTitleBarSwitch />
        </SettingsSection>
      )}
    </>
  )
}

/** Zoom Out · the level · Zoom In, then Actual Size: the items of the View menu, which run the same command */
function ZoomStepper() {
  const zoom = useAppSettings(state => state.zoom)
  const items = new Map(zoomMenuItems().map(item => [item.id, item]))
  return (
    <div className="flex items-center gap-2" data-testid="setting-zoom">
      <FieldGroup>
        <ZoomButton item={items.get("zoom_out")!} />
        <span className="inline-flex h-7 min-w-14 items-center justify-center border border-input px-2 text-sm tabular-nums" data-testid="setting-zoom-level">
          {`${Math.round(zoom * 100)}%`}
        </span>
        <ZoomButton item={items.get("zoom_in")!} />
      </FieldGroup>
      <ZoomButton item={items.get("zoom_reset")!} />
    </div>
  )
}

/** An item of the View menu as a button: its icon (else its label), its keys in the tooltip */
function ZoomButton({ item }: { item: MenuItem }) {
  const Icon = item.icon
  return (
    <Tooltip>
      <TooltipTrigger
        render={<Button variant="outline" size={Icon === undefined ? "sm" : "icon-sm"} aria-label={Icon === undefined ? undefined : item.label} onClick={item.onSelect} />}
      >
        {Icon === undefined ? item.label : <Icon />}
      </TooltipTrigger>
      <TooltipContent className="flex items-center gap-1.5">
        {item.label}
        {item.kbds !== undefined && (
          <KbdGroup>
            {item.kbds.map(kbd => (
              <Kbd key={kbd}>{kbdLabel(kbd)}</Kbd>
            ))}
          </KbdGroup>
        )}
      </TooltipContent>
    </Tooltip>
  )
}

/** The layout of the diffs, the switch above every diff too, and soft wraps */
function DiffSettings() {
  const softWraps = useDiffSoftWraps()
  return (
    <div className="flex flex-col gap-3">
      <div data-testid="setting-diffViewMode">
        <DiffViewToggle />
      </div>
      <SettingRow
        testId="setting-softWraps"
        label="Soft wraps"
        description="Break a long line at the edge of the diff; without wraps it scrolls, both sides of Split together"
        checked={softWraps}
        onCheckedChange={setDiffSoftWraps}
      />
    </div>
  )
}

/** Why the system title bar stays whatever the setting says (`titleBar.reason`) */
const fixedTitleBarReasons: Partial<Record<NativeTitleBarReason, string>> = {
  wsl: "WSL draws the frame of its windows",
  "tiling-wm": "A tiling window manager draws the titles",
  "no-desktop": "The desktop is unknown (no XDG_CURRENT_DESKTOP)",
  env: "Set by BRANCH_DECK_NATIVE_TITLE_BAR",
}

/**
 * The system title bar and menu bar instead of the page's (IntelliJ's "Merge main menu with window title", inverted).
 * The backend decides at startup, so a change applies after a restart; where the system title bar is fixed the switch
 * is on, disabled, and says why.
 */
function SystemTitleBarSwitch() {
  const setting = useAppSettings(state => state.systemTitleBar)
  const fixedReason = titleBar.kind === "native" && titleBar.reason !== undefined ? fixedTitleBarReasons[titleBar.reason] : undefined
  // what the windows show now: the setting applies to the next start
  const showsSystem = titleBar.kind === "native"
  const pending = fixedReason === undefined && setting !== showsSystem
  return (
    <div className="flex flex-col gap-2">
      <SettingRow
        testId="setting-systemTitleBar"
        label="Use the system title bar"
        description={fixedReason ?? "The title bar and the menu bar of the system, instead of the window buttons and ☰ in the toolbar"}
        checked={fixedReason !== undefined || setting}
        disabled={fixedReason !== undefined}
        onCheckedChange={setSystemTitleBar}
      />
      {pending && (
        <p className="flex items-center gap-2 pl-10 text-xs text-muted-foreground" data-testid="setting-systemTitleBar-restart">
          Applies after a restart
          <Button
            size="xs"
            variant="outline"
            onClick={() => {
              // the backend writes the setting before it restarts: the main window saves the settings debounced
              commands
                .restartApp(setting)
                .then(result => {
                  if (result.status === "error") {
                    notifyError("Cannot restart Branch Deck", result.error)
                  }
                })
                .catch((error: unknown) => notifyError("Cannot restart Branch Deck", error))
            }}
          >
            <RotateCwIcon data-icon="inline-start" />
            Restart
          </Button>
        </p>
      )}
    </div>
  )
}
