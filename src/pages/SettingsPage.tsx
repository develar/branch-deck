import { useEffect, useEffectEvent, useRef, useState } from "react"
import type { ReactNode } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AccountsPage } from "@/components/settings/AccountsPage"
import { AgentsPage } from "@/components/settings/AgentsPage"
import { AppearancePage } from "@/components/settings/AppearancePage"
import { BranchesPage } from "@/components/settings/BranchesPage"
import { DiffsPage } from "@/components/settings/DiffsPage"
import { GeneralPage } from "@/components/settings/GeneralPage"
import { LandingPage } from "@/components/settings/LandingPage"
import { NotificationsPage } from "@/components/settings/NotificationsPage"
import { ScopeTabs } from "@/components/settings/ScopeTabs"
import { ToolsPage } from "@/components/settings/ToolsPage"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { SettingsScopeContext } from "@/hooks/useSettingsScope"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { logError } from "@/lib/errors"
import { repositoryName } from "@/lib/repository-config"
import { saveAllBeforeClose } from "@/lib/settings-close"
import type { SettingsPageId } from "@/lib/settings-pages"
import { REPOSITORY_PAGES, SETTINGS_PAGES } from "@/lib/settings-pages"
import { resolveSettingsPage, resolveSettingsScope } from "@/lib/settings-window"
import type { SettingsScope, SettingsTarget } from "@/lib/settings-window"
import { useAuthStatusOnOpen } from "@/stores/auth"
import { useSelectedProjectPath } from "@/stores/repository"
import { appSettingsStore, setSettingsPage } from "@/stores/settings/appSettings"
import { useSetupEntries, useSetupTracking } from "@/stores/setup"

const PAGE_LABELS: Record<SettingsPageId, string> = {
  general: "General",
  appearance: "Appearance",
  diffs: "Diffs",
  notifications: "Notifications",
  accounts: "Accounts",
  tools: "Tools",
  branches: "Branches",
  landing: "Landing",
  agents: "Agents",
}

/** The groups of the sidebar: the settings of the app, then the git config of the repositories, under the scope tabs */
const PAGE_GROUPS: ReadonlyArray<{ label: string; pages: readonly SettingsPageId[] }> = [
  { label: "Branch Deck", pages: SETTINGS_PAGES.filter(id => !REPOSITORY_PAGES.includes(id)) },
  { label: "Repositories", pages: REPOSITORY_PAGES },
]

/** How long an open waits for the field it names to show (a page that reads its settings first) */
const FOCUS_WAIT_MS = 5000

/**
 * The Settings window (openspec/specs/repository/settings/spec.md): the pages in a sidebar on the canvas, in two groups, the page
 * in one island; the pages of the git config under the scope tabs. ↑/↓ on the sidebar switch the page. An open shows
 * the page it names (`SettingsTarget`), else the page shown last (`appSettings.settingsPage`), at the scope it names,
 * else "All repositories", and focuses the field it names, else the tab of the page.
 */
export function SettingsPage() {
  const target = useSubWindowData<SettingsTarget>()
  const path = useSelectedProjectPath() ?? ""
  const [page, setPage] = useState(() => resolveSettingsPage(appSettingsStore.getState().settingsPage, target))
  const [scope, setScope] = useState<SettingsScope>(() => resolveSettingsScope(target))
  // each open of the window, also of the open one: the page it names, its scope, the field to focus, the checks of the pages
  const [seenTarget, setSeenTarget] = useState(target)
  const [openCount, setOpenCount] = useState(0)
  if (target !== seenTarget) {
    setSeenTarget(target)
    setOpenCount(openCount + 1)
    setPage(resolveSettingsPage(page, target))
    setScope(resolveSettingsScope(target))
  }
  // without a repository there is only the value of every repository
  const shownScope: SettingsScope = path === "" ? "all" : scope
  useSaveBeforeClose()
  // where the window opens next time
  useEffect(() => {
    setSettingsPage(page)
  }, [page])

  const listRef = useRef<HTMLDivElement>(null)
  const focusOpened = useEffectEvent(() => {
    // the field of the page shown: the page left may still be mounted while its transition ends
    const selector = target?.focus === undefined ? null : `[data-testid="settings-page-${page}"] [data-settings-focus="${CSS.escape(target.focus)}"]`
    const field = selector === null ? null : focusableField(selector)
    if (field !== null) {
      field.focus()
      return () => {}
    }
    const list = listRef.current
    const tab = list?.querySelector<HTMLElement>("[role=tab][aria-selected=true]")
    const stopTab = list === null || tab === null || tab === undefined ? () => {} : focusWhenTabbable(list, tab)
    // the field of a page that reads its settings first (Branches, Landing) is enabled once they answer
    const stopField = selector === null ? () => {} : focusWhenShown(selector)
    return () => {
      stopTab()
      stopField()
    }
  })
  useEffect(() => focusOpened(), [openCount])
  // the sign-ins of GitHub and YouTrack: a token of gh or of the environment may have come meanwhile
  useAuthStatusOnOpen(openCount)
  useSetupTracking({ readTools: false })
  const setupEntries = useSetupEntries()
  const needsYou: Partial<Record<SettingsPageId, boolean>> = {
    tools: setupEntries.some(entry => "tool" in entry.item),
    accounts: setupEntries.some(entry => entry.item.kind === "youTrackSignIn"),
  }
  const scopeValue = { scope: shownScope, setScope, path, name: path === "" ? "" : repositoryName(path) }

  return (
    <WindowPage
      title="Settings"
      body="fill"
      data-testid="settings-window"
      footer={
        <PreviewDialogFooter className="border-t-0 px-1 py-0">
          <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
        </PreviewDialogFooter>
      }
    >
      <Tabs orientation="vertical" value={page} onValueChange={value => setPage(value as SettingsPageId)} className="min-h-0 flex-1 gap-2">
        <TabsList ref={listRef} variant="list" activateOnFocus aria-label="Settings pages" className="w-44 shrink-0 self-start">
          {PAGE_GROUPS.map(group => [
            <span key={group.label} className="px-2 pt-1.5 pb-0.5 text-xs text-muted-foreground first:pt-0" aria-hidden>
              {group.label}
            </span>,
            ...group.pages.map(id => (
              <TabsTrigger key={id} value={id} data-testid={`settings-tab-${id}`}>
                {PAGE_LABELS[id]}
                {needsYou[id] === true && <span className="size-1.5 rounded-full bg-primary" data-testid="setup-dot" aria-hidden />}
              </TabsTrigger>
            )),
          ])}
        </TabsList>
        {/* one island; it scrolls only when a large zoom leaves the page no room */}
        <Card className="min-h-0 flex-1 overflow-y-auto" data-testid="settings-island">
          <SettingsPanel id="general">
            <GeneralPage />
          </SettingsPanel>
          <SettingsPanel id="appearance">
            <AppearancePage />
          </SettingsPanel>
          <SettingsPanel id="diffs">
            <DiffsPage />
          </SettingsPanel>
          <SettingsPanel id="notifications">
            <NotificationsPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="accounts">
            <AccountsPage />
          </SettingsPanel>
          <SettingsPanel id="tools">
            <ToolsPage openCount={openCount} server={target?.server} />
          </SettingsPanel>
          <SettingsScopeContext value={scopeValue}>
            {/* the scope tabs of the pages of the git config, once above the page shown */}
            {REPOSITORY_PAGES.includes(page) && (
              <div className="px-(--card-spacing)">
                <ScopeTabs />
              </div>
            )}
            {/* one page per repository and scope: what was typed for one is never written to another */}
            <SettingsPanel id="branches">
              <BranchesPage key={`${path}:${shownScope}`} />
            </SettingsPanel>
            <SettingsPanel id="landing">
              <LandingPage key={`${path}:${shownScope}`} />
            </SettingsPanel>
            <SettingsPanel id="agents">
              <AgentsPage key={`${path}:${shownScope}`} openCount={openCount} />
            </SettingsPanel>
          </SettingsScopeContext>
        </Card>
      </Tabs>
    </WindowPage>
  )
}

/**
 * Saves the typing of every field before the window closes (⌘W, the close button): one listener, which waits for the
 * saves the fields registered (`saveBeforeClose`)
 */
function useSaveBeforeClose(): void {
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    let cancelled = false
    getCurrentWindow()
      // a failed save never keeps the window open: Tauri destroys it only after the handler resolved
      .onCloseRequested(() => saveAllBeforeClose().catch((error: unknown) => logError("Cannot save the settings before the window closes", error)))
      .then(
        fn => {
          if (cancelled) {
            fn()
          } else {
            unlisten = fn
          }
        },
        (error: unknown) => logError("Cannot save the settings when the window closes", error),
      )
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])
}

/**
 * Focuses the tab once Base UI registered the tabs of the list (one of them is tabbable): a tab focused before keeps a
 * stale highlight, and the first ↓ would go nowhere. Returns the cleanup of the wait.
 */
function focusWhenTabbable(list: HTMLElement, tab: HTMLElement): () => void {
  const registered = () => list.querySelector("[role=tab][tabindex='0']") !== null
  if (registered()) {
    tab.focus()
    return () => {}
  }
  const observer = new MutationObserver(() => {
    if (registered()) {
      observer.disconnect()
      tab.focus()
    }
  })
  observer.observe(list, { subtree: true, attributes: true, attributeFilter: ["tabindex"] })
  return () => observer.disconnect()
}

/** The element of `selector` once it can take the focus: shown, and a field no longer disabled while its settings load */
function focusableField(selector: string): HTMLElement | null {
  const element = document.querySelector<HTMLElement>(selector)
  return element === null || element.matches(":disabled") ? null : element
}

/**
 * Focuses the element of `selector` once it can take the focus (`focusableField`), for at most `FOCUS_WAIT_MS` (a
 * field the page does not have never shows); returns the cleanup of the wait
 */
function focusWhenShown(selector: string): () => void {
  const observer = new MutationObserver(() => {
    const element = focusableField(selector)
    if (element !== null) {
      stop()
      element.focus()
    }
  })
  const timer = setTimeout(() => stop(), FOCUS_WAIT_MS)
  function stop() {
    clearTimeout(timer)
    observer.disconnect()
  }
  observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["disabled"] })
  return stop
}

/** A page in the island; the page left stays mounted while Base UI ends its transition, hidden, so the two never stack */
function SettingsPanel({ id, children }: { id: SettingsPageId; children: ReactNode }) {
  return (
    <TabsContent value={id} data-testid={`settings-page-${id}`} className="flex flex-col gap-5 px-(--card-spacing) data-ending-style:hidden">
      {children}
    </TabsContent>
  )
}
