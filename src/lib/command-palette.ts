import type { LucideIcon } from "lucide-react"
import { AppWindowIcon, GitBranchIcon, HistoryIcon, LinkIcon } from "lucide-react"
import { mainMenus } from "@/lib/main-menu"
import type { MainMenuState } from "@/lib/main-menu"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { findMenuItem, FROM_MENU } from "@/lib/menu"
import type { MainViewEntry } from "@/stores/main-view"

/*
 * The command palette of the main window (⌘K, spec/repository/command-palette.spec.md): a way to where the user wants
 * to be, to the commands of the app and to the actions of the branch at hand. Its sources, in their order: the views of
 * the window (`MAIN_VIEWS`, which a view joins by its entry), Activity, the commands of the app's menu (the items of
 * `mainMenus`, so the palette offers them as the menu does), the actions of the branch whose row has the focus (the
 * items of its menu, `getBranchContextMenuItems`, which present the actions Rust decides) and "Go to <branch>" for every
 * branch of the table.
 */

export type PaletteGroup = "Views" | "Activity" | "Commands" | "Branch" | "Branches"

export const PALETTE_GROUPS: readonly PaletteGroup[] = ["Views", "Activity", "Commands", "Branch", "Branches"]

export interface PaletteCommand {
  /** Namespaced by its source (`view:branches`, `activity:panel`, `app:sync_branches`, `branch:auth`): one entry per id */
  readonly id: string
  readonly label: string
  readonly group: PaletteGroup
  /** The branch "Go to <branch>" goes to: a search that starts with its name ranks it first (`paletteGroups`) */
  readonly branch?: string
  readonly icon?: LucideIcon
  /** The key of the command anywhere in the window; a command without a global key has none */
  readonly kbds?: readonly string[]
  readonly disabled?: boolean
  /** Why it is disabled */
  readonly description?: string
  /** Runs once the palette closed and gave the focus back */
  readonly run: () => void
}

/** What the commands of the palette are built from, and what they run */
export interface PaletteSources {
  readonly views: readonly MainViewEntry[]
  readonly showView: (entry: MainViewEntry) => void
  /** The state of the main menu (Sync and its reason, whether the build updates itself) */
  readonly menu: MainMenuState
  /** The branches of the table, in its order */
  readonly branches: readonly string[]
  readonly toggleActivityPanel: () => void
  readonly openActivityWindow: () => void
  readonly goToBranch: (name: string) => void
  /** The branch whose row has the focus, with the groups of its menu; null without one */
  readonly branchMenu: { readonly name: string; readonly groups: MenuItemGroups } | null
  /** Shows Reviews with the focus in its field of a link: a pull request the author forgot to ask the user for */
  readonly addPullRequestByLink?: () => void
}

/**
 * The actions of the branch `name` from the groups of its menu, each as the menu has it (its name, disabled with its
 * reason), an item of a submenu named with it ("Patronus › Dry Run"); a submenu with nothing to act on is none. A key
 * of the row is no key of the window: none is hinted
 */
function branchActions(name: string, groups: MenuItemGroups): PaletteCommand[] {
  const command = (item: MenuItem, path: string, label: string): PaletteCommand => ({
    id: `action:${name}:${path}`,
    label,
    group: "Branch",
    ...(item.icon !== undefined && { icon: item.icon }),
    ...(item.disabled === true && { disabled: true }),
    ...(item.disabled === true && item.description !== undefined && { description: item.description }),
    run: () => item.onSelect(FROM_MENU),
  })
  return groups.flat().flatMap(item => {
    const path = item.id ?? item.label
    if (item.children === undefined) {
      return [command(item, path, item.label)]
    }
    return item.children
      .filter(child => child.children === undefined)
      .map(child => command(child, `${path}/${child.id ?? child.label}`, `${item.label.replace(/…$/, "")} › ${child.label}`))
  })
}

/** The commands of the app's menu the palette offers: what the user does with the app, never a view setting */
const MENU_COMMANDS = ["sync_branches", "update_project", "open_settings", "install_agent_cli", "check_for_updates"] as const

/** The commands of the palette, in the order of their groups; an id a source gave before is left out */
export function paletteCommands(sources: PaletteSources): PaletteCommand[] {
  const menus = mainMenus(sources.menu).map(menu => menu.groups)
  const commands: PaletteCommand[] = [
    ...sources.views.map(entry => ({
      id: `view:${entry.view}`,
      label: entry.label,
      group: "Views" as const,
      icon: entry.icon,
      ...(entry.kbds !== undefined && { kbds: entry.kbds }),
      run: () => sources.showView(entry),
    })),
    { id: "activity:panel", label: "Activity Panel", group: "Activity", icon: HistoryIcon, run: sources.toggleActivityPanel },
    { id: "activity:window", label: "Activity Window", group: "Activity", icon: AppWindowIcon, run: sources.openActivityWindow },
    ...(sources.addPullRequestByLink === undefined
      ? []
      : [{ id: "reviews:link", label: "Add Pull Request by Link…", group: "Commands" as const, icon: LinkIcon, run: sources.addPullRequestByLink }]),
    ...MENU_COMMANDS.flatMap(id => {
      const item = menus.map(groups => findMenuItem(groups, id)).find(found => found !== undefined)
      if (item === undefined) {
        return []
      }
      return [
        {
          id: `app:${id}`,
          label: item.label,
          group: "Commands" as const,
          ...(item.icon !== undefined && { icon: item.icon }),
          ...(item.kbds !== undefined && { kbds: item.kbds }),
          ...(item.disabled === true && { disabled: true }),
          ...(item.description !== undefined && { description: item.description }),
          run: () => item.onSelect(FROM_MENU),
        },
      ]
    }),
    ...(sources.branchMenu === null ? [] : branchActions(sources.branchMenu.name, sources.branchMenu.groups)),
    ...sources.branches.map(name => ({
      id: `branch:${name}`,
      label: `Go to ${name}`,
      group: "Branches" as const,
      branch: name,
      icon: GitBranchIcon,
      run: () => sources.goToBranch(name),
    })),
  ]
  const seen = new Set<string>()
  return commands.filter(command => {
    if (seen.has(command.id)) {
      return false
    }
    seen.add(command.id)
    return true
  })
}

/** A group of the palette with its commands, as the list shows them */
export interface PaletteCommandGroup {
  readonly value: PaletteGroup
  readonly items: readonly PaletteCommand[]
}

/**
 * The groups of the palette in the order the list shows them for the typed `text` (the filter of the list keeps the
 * commands whose label has it): without a text in the order of `PALETTE_GROUPS`; with one, a command that starts with
 * it first (its label does, or the name of the branch of a Go to), in its group and its group before the others. So the
 * name of a branch typed and Enter go to the branch, not to an action of the branch at hand whose label names it
 * ("Push session Only", "Stack On › session"), which is listed first otherwise and which Enter would run.
 * `startsWith` compares as the filter of the list does (`Combobox.useFilter`: case and accents aside).
 */
export function paletteGroups(commands: readonly PaletteCommand[], text: string, startsWith: (value: string, query: string) => boolean): PaletteCommandGroup[] {
  const groups = PALETTE_GROUPS.map(group => ({ value: group, items: commands.filter(command => command.group === group) })).filter(group => group.items.length > 0)
  if (text === "") {
    return groups
  }
  const leads = (command: PaletteCommand) => startsWith(command.label, text) || (command.branch !== undefined && startsWith(command.branch, text))
  const tiered = groups.map(group => {
    const first = group.items.filter(leads)
    return { value: group.value, items: [...first, ...group.items.filter(command => !first.includes(command))], leads: first.length > 0 }
  })
  return [...tiered.filter(group => group.leads), ...tiered.filter(group => !group.leads)].map(({ value, items }) => ({ value, items }))
}
