import { useRef, useState } from "react"
import { Combobox as ComboboxPrimitive } from "@base-ui/react/combobox"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Combobox, ComboboxCollection, ComboboxEmpty, ComboboxGroup, ComboboxInput, ComboboxItem, ComboboxLabel, ComboboxList } from "@/components/ui/combobox"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { openActivityInWindow } from "@/hooks/useActivityPanel"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useBranchMenu } from "@/hooks/useBranchContextActions"
import { useSyncAvailability } from "@/hooks/useSyncAvailability"
import { openActivity } from "@/lib/activity-window"
import { paletteCommands, paletteGroups } from "@/lib/command-palette"
import type { PaletteCommand, PaletteCommandGroup } from "@/lib/command-palette"
import { logError } from "@/lib/errors"
import { kbdLabel } from "@/lib/menu"
import { titleBar } from "@/lib/title-bar"
import { useBranchSync } from "@/stores/branch-sync"
import { getSelectedBranches, requestReveal, selectBranches } from "@/stores/commit-move"
import { MAIN_VIEWS, showMainView } from "@/stores/main-view"
import { requestLinkFieldFocus } from "@/stores/reviews"
import { useAppSettings } from "@/stores/settings/appSettings"

/** Expands the row of a branch an action of the palette shows the commits of (Amend Changes…, Unapply…) */
function expandRow(name: string, expanded: boolean): void {
  if (expanded) {
    requestReveal({ branchName: name, commitIds: [] })
  }
}

/** The branch the actions of the palette act on: the one whose row has the focus, else the one branch selected */
function branchAtHand(focused: HTMLElement | null): string | null {
  const row = focused?.closest<HTMLElement>("tr[data-row-id]")?.dataset.rowId
  if (row !== undefined) {
    return row
  }
  const selected = getSelectedBranches()
  return selected.length === 1 ? selected[0]! : null
}

/**
 * Shows the branch: its row expanded, scrolled into view and focused (`requestReveal`), also a row a collapsed stack or
 * the filter hid, which the reveal renders first; selected, so its keys work
 */
function goToBranch(name: string): void {
  requestReveal({ branchName: name, commitIds: [], focus: true })
  selectBranches(name, "only", [])
}

/**
 * The command palette of the main window (spec/repository/command-palette.spec.md): ⌘K (View → Command Palette…, the
 * app command `open_command_palette`) opens a dialog with a search over the views, Activity, the commands of the app,
 * the actions of the branch whose row has the focus (its menu, as Rust decides the actions) and the branches of the
 * table. A command runs once the palette closed and gave the focus back where it was, so what
 * the command focuses (a row, the Activity panel) keeps it; Escape closes it with nothing run.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false)
  // what the search holds: a command that starts with it comes first (`paletteGroups`)
  const [text, setText] = useState("")
  const { startsWith } = ComboboxPrimitive.useFilter()
  const sync = useSyncAvailability()
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const branches = useBranchSync(state => state.branchOrder)
  // the branch at hand when the palette opened, and its menu as the stores have it while the palette shows
  const [actionBranch, setActionBranch] = useState<string | null>(null)
  const branchGroups = useBranchMenu(actionBranch ?? "", expandRow)
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
      setActionBranch(branchAtHand(focusBefore.current))
      setText("")
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
        branchMenu: actionBranch === null || branchGroups === null ? null : { name: actionBranch, groups: branchGroups },
        addPullRequestByLink: () => {
          requestLinkFieldFocus()
          showMainView("reviews")
        },
      })
    : []
  const groups = paletteGroups(commands, text, startsWith)

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
          onInputValueChange={setText}
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
            {(group: PaletteCommandGroup) => (
              <ComboboxGroup key={group.value} items={group.items}>
                <ComboboxLabel>{group.value === "Branch" && actionBranch !== null ? actionBranch : group.value}</ComboboxLabel>
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
