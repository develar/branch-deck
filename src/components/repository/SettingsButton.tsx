import { useState } from "react"
import { SettingsIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ShortcutHint } from "@/components/shared/ShortcutHint"
import { isMac } from "@/lib/platform"
import { openSettings } from "@/lib/settings-window"
import { describeSetupItem, settingsButtonLabel } from "@/lib/setup"
import { runSetupItem, skipSetupEntry } from "@/lib/setup-actions"
import { useSetupEntries } from "@/stores/setup"

/**
 * Settings in the header: a click opens it; while something of the setup waits for the user (the Setup dot,
 * spec/repository/setup-attention.spec.md) the button has a dot and a click lists it first, each with its action and
 * its skip, then Settings…
 */
export function SettingsButton() {
  const entries = useSetupEntries()
  const [open, setOpen] = useState(false)
  const label = settingsButtonLabel(entries.length)
  if (entries.length === 0) {
    return (
      <Tooltip>
        <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={label} data-testid="settings-button" onClick={() => openSettings()} />}>
          <SettingsIcon />
        </TooltipTrigger>
        <TooltipContent>
          <ShortcutHint kbd={isMac ? "⌘," : "Ctrl+,"}>Settings</ShortcutHint>
        </TooltipContent>
      </Tooltip>
    )
  }
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Button variant="ghost" size="icon-sm" aria-label={label} data-testid="settings-button" className="relative" />}>
        <SettingsIcon />
        <span className="absolute top-1 right-1 size-1.5 rounded-full bg-primary" data-testid="setup-dot" aria-hidden />
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 gap-3" data-testid="setup-popover">
        <ul className="flex flex-col gap-3">
          {entries.map(entry => {
            const words = describeSetupItem(entry)
            return (
              <li key={entry.key} className="flex flex-col gap-1.5" data-testid="setup-item">
                <span>{words.title}</span>
                <span className="flex items-center gap-1">
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() => {
                      setOpen(false)
                      runSetupItem(entry)
                    }}
                  >
                    {words.action}
                  </Button>
                  {words.skip !== null && (
                    <Button size="xs" variant="ghost" onClick={() => skipSetupEntry(entry)}>
                      {words.skip}
                    </Button>
                  )}
                </span>
              </li>
            )
          })}
        </ul>
        <Button
          variant="ghost"
          size="sm"
          className="self-start"
          onClick={() => {
            setOpen(false)
            openSettings()
          }}
        >
          <SettingsIcon data-icon="inline-start" />
          Settings…
        </Button>
      </PopoverContent>
    </Popover>
  )
}
