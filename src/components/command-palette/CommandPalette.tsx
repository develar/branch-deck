import { useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Combobox, ComboboxCollection, ComboboxEmpty, ComboboxGroup, ComboboxInput, ComboboxItem, ComboboxLabel, ComboboxList } from "@/components/ui/combobox"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { openActivityInWindow } from "@/hooks/useActivityPanel"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useSyncAvailability } from "@/hooks/useSyncAvailability"
import { openActivity } from "@/lib/activity-window"
import { PALETTE_GROUPS, paletteCommands } from "@/lib/command-palette"
import type { PaletteCommand, PaletteGroup } from "@/lib/command-palette"
import { logError } from "@/lib/errors"
import { kbdLabel } from "@/lib/menu"
import { titleBar } from "@/lib/title-bar"
import { useBranchSync } from "@/stores/branch-sync"
import { requestReveal, selectBranches } from "@/stores/commit-move"
import { MAIN_VIEWS, showMainView } from "@/stores/main-view"
import { useAppSettings } from "@/stores/settings/appSettings"

interface CommandGroup {
  readonly value: PaletteGroup
  readonly items: readonly PaletteCommand[]
}

/** Shows the branch: its row expanded and scrolled into view (`requestReveal`), selected and focused, so its keys work */
function goToBranch(name: string): void {
  requestReveal({ branchName: name, commitIds: [] })
  selectBranches(name, "only", [])
  document.querySelector<HTMLElement>(`tr[data-row-id="${CSS.escape(name)}"]`)?.focus({ preventScroll: true })
}

/**
 * The command palette of the main window (spec/repository/command-palette.spec.md): ⌘K (View → Command Palette…, the
 * app command `open_command_palette`) opens a dialog with a search over the views, Activity, the commands of the app
 * and the branches of the table. A command runs once the palette closed and gave the focus back where it was, so what
 * the command focuses (a row, the Activity panel) keeps it; Escape closes it with nothing run.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false)
  const sync = useSyncAvailability()
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const branches = useBranchSync(state => state.branchOrder)
  // the element focused before the palette, and the command chosen in it
  const focusBefore = useRef<HTMLElement | null>(null)
  const chosen = useRef<PaletteCommand | null>(null)

  // focuses the element focused before itself, not its first tabbable one the default would take (a row is focusable,
  // not tabbable); false without it
  const restoreFocus = (): boolean => {
    const before = focusBefore.current
    if (before?.isConnected !== true) {
      return false
    }
    before.focus({ preventScroll: true })
    return true
  }

  // ⌘K of every window comes here (the native menu sends it to the main window): the window comes to the front
  useAppCommand("open_command_palette", () => {
    getCurrentWindow()
      .setFocus()
      .catch((error: unknown) => logError("Cannot focus the main window", error))
    if (!open) {
      focusBefore.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
      chosen.current = null
      setOpen(true)
    }
  })

  const commands = open
    ? paletteCommands({
        views: MAIN_VIEWS,
        showView: entry => showMainView(entry.view),
        menu: { sync, autoSyncOnFocus, updates: titleBar.kind === "custom" && titleBar.checkForUpdates },
        branches,
        toggleActivityPanel: () => openActivity(),
        openActivityWindow: () => openActivityInWindow(null),
        goToBranch,
      })
    : []
  const groups: CommandGroup[] = PALETTE_GROUPS.map(group => ({ value: group, items: commands.filter(command => command.group === group) })).filter(group => group.items.length > 0)

  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      onOpenChangeComplete={isOpen => {
        const command = chosen.current
        if (isOpen || command === null) {
          return
        }
        // the choice stays until the next open: the focus of the close (`finalFocus`) is decided after this
        restoreFocus()
        command.run()
      }}
    >
      <DialogContent
        showCloseButton={false}
        className="top-[20%] translate-y-0 gap-0 p-0 sm:max-w-lg"
        // a chosen command decides where the focus goes; closed without one, the focus goes back where it was
        finalFocus={() => chosen.current === null && !restoreFocus()}
        data-testid="command-palette"
      >
        <DialogTitle className="sr-only">Command Palette</DialogTitle>
        <Combobox<PaletteCommand>
          inline
          open={open}
          autoHighlight
          items={groups}
          value={null}
          itemToStringLabel={command => command.label}
          onValueChange={command => {
            if (command !== null && command.disabled !== true) {
              chosen.current = command
              setOpen(false)
            }
          }}
        >
          <ComboboxInput showTrigger={false} placeholder="Type a command or a branch" aria-label="Command or branch" className="m-2 h-8" />
          <ComboboxEmpty>No command or branch matches</ComboboxEmpty>
          <ComboboxList className="max-h-80 border-t border-border">
            {(group: CommandGroup) => (
              <ComboboxGroup key={group.value} items={group.items}>
                <ComboboxLabel>{group.value}</ComboboxLabel>
                <ComboboxCollection>
                  {(command: PaletteCommand) => (
                    <ComboboxItem key={command.id} value={command} disabled={command.disabled} className="pr-2" data-command-id={command.id}>
                      {command.icon !== undefined && <command.icon />}
                      <span className="truncate">{command.label}</span>
                      {command.description !== undefined && <span className="truncate text-xs text-muted-foreground">{command.description}</span>}
                      {command.kbds !== undefined && (
                        <KbdGroup className="ml-auto">
                          {command.kbds.map(kbd => (
                            <Kbd key={kbd}>{kbdLabel(kbd)}</Kbd>
                          ))}
                        </KbdGroup>
                      )}
                    </ComboboxItem>
                  )}
                </ComboboxCollection>
              </ComboboxGroup>
            )}
          </ComboboxList>
        </Combobox>
      </DialogContent>
    </Dialog>
  )
}
