import { Fragment, useEffect, useEffectEvent, useRef, useState } from "react"
import type { ReactNode } from "react"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AgentsPage } from "@/components/settings/AgentsPage"
import { AppearancePage } from "@/components/settings/AppearancePage"
import { DiffsPage } from "@/components/settings/DiffsPage"
import { GeneralPage } from "@/components/settings/GeneralPage"
import { GitHubPage } from "@/components/settings/GitHubPage"
import { NotificationsPage } from "@/components/settings/NotificationsPage"
import { PatronusPage } from "@/components/settings/PatronusPage"
import { RepositoryPage } from "@/components/settings/RepositoryPage"
import { ToolsPage } from "@/components/settings/ToolsPage"
import { YouTrackPage } from "@/components/settings/YouTrackPage"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { repositoryName } from "@/lib/repository-config"
import type { SettingsPageId } from "@/lib/settings-pages"
import { SETTINGS_PAGES } from "@/lib/settings-pages"
import { resolveSettingsPage } from "@/lib/settings-window"
import type { SettingsTarget } from "@/lib/settings-window"
import { useSelectedProjectPath } from "@/stores/repository"
import { appSettingsStore, setSettingsPage } from "@/stores/settings/appSettings"
import { useSetupEntries, useSetupTracking } from "@/stores/setup"

const PAGE_LABELS: Record<SettingsPageId, string> = {
  general: "General",
  appearance: "Appearance",
  diffs: "Diffs & Merge",
  notifications: "Notifications",
  github: "GitHub",
  youtrack: "YouTrack",
  patronus: "Patronus",
  agents: "Agents",
  tools: "Tools",
  repository: "Repository",
}

/**
 * The Settings window (spec/repository/settings.spec.md): the pages in a sidebar on the canvas, the page in one island.
 * ↑/↓ on the sidebar switch the page. An open shows the page it names (`SettingsTarget`), else the page shown last
 * (`appSettings.settingsPage`), and focuses the field it names, else the tab of the page.
 */
export function SettingsPage() {
  const target = useSubWindowData<SettingsTarget>()
  const [page, setPage] = useState(() => resolveSettingsPage(appSettingsStore.getState().settingsPage, target))
  // each open of the window, also of the open one: the page it names, the field to focus, the checks of the pages
  const [seenTarget, setSeenTarget] = useState(target)
  const [openCount, setOpenCount] = useState(0)
  if (target !== seenTarget) {
    setSeenTarget(target)
    setOpenCount(openCount + 1)
    setPage(resolveSettingsPage(page, target))
  }
  // where the window opens next time
  useEffect(() => {
    setSettingsPage(page)
  }, [page])

  const listRef = useRef<HTMLDivElement>(null)
  const focusOpened = useEffectEvent(() => {
    // the field of the page shown: the page left may still be mounted while its transition ends
    const selector = target?.focus === undefined ? null : `[data-testid="settings-page-${page}"] [data-settings-focus="${CSS.escape(target.focus)}"]`
    const field = selector === null ? null : document.querySelector<HTMLElement>(selector)
    if (field !== null) {
      field.focus()
      return () => {}
    }
    const list = listRef.current
    const tab = list?.querySelector<HTMLElement>("[role=tab][aria-selected=true]")
    const stopTab = list === null || tab === null || tab === undefined ? () => {} : focusWhenTabbable(list, tab)
    // the field of a page that reads its settings first (Repository) shows once they answer
    const stopField = selector === null ? () => {} : focusWhenShown(selector)
    return () => {
      stopTab()
      stopField()
    }
  })
  useEffect(() => focusOpened(), [openCount])
  useSetupTracking({ readTools: false })
  const toolsNeedYou = useSetupEntries().some(entry => "tool" in entry.item)

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
        <TabsList ref={listRef} variant="list" activateOnFocus aria-label="Settings pages" className="w-40 shrink-0 self-start">
          {SETTINGS_PAGES.map(id =>
            id === "repository" ? (
              <Fragment key={id}>
                <Separator className="my-1.5" />
                <RepositoryTab />
              </Fragment>
            ) : (
              <TabsTrigger key={id} value={id} data-testid={`settings-tab-${id}`}>
                {PAGE_LABELS[id]}
                {id === "tools" && toolsNeedYou && <span className="size-1.5 rounded-full bg-primary" data-testid="setup-dot" aria-hidden />}
              </TabsTrigger>
            ),
          )}
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
          <SettingsPanel id="github">
            <GitHubPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="youtrack">
            <YouTrackPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="patronus">
            <PatronusPage />
          </SettingsPanel>
          <SettingsPanel id="agents">
            <AgentsPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="tools">
            <ToolsPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="repository">
            <RepositoryPage />
          </SettingsPanel>
        </Card>
      </Tabs>
    </WindowPage>
  )
}

/** The tab of the page of the selected repository, its name muted beside the page's (the page says it again) */
function RepositoryTab() {
  const path = useSelectedProjectPath()
  return (
    <TabsTrigger value="repository" data-testid="settings-tab-repository" aria-label={PAGE_LABELS.repository}>
      {PAGE_LABELS.repository}
      {path !== undefined && <span className="min-w-0 truncate text-xs text-muted-foreground">{repositoryName(path)}</span>}
    </TabsTrigger>
  )
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

/** Focuses the element of `selector` once it shows; returns the cleanup of the wait */
function focusWhenShown(selector: string): () => void {
  const observer = new MutationObserver(() => {
    const element = document.querySelector<HTMLElement>(selector)
    if (element !== null) {
      observer.disconnect()
      element.focus()
    }
  })
  observer.observe(document.body, { subtree: true, childList: true })
  return () => observer.disconnect()
}

/** A page in the island; the page left stays mounted while Base UI ends its transition, hidden, so the two never stack */
function SettingsPanel({ id, children }: { id: SettingsPageId; children: ReactNode }) {
  return (
    <TabsContent value={id} data-testid={`settings-page-${id}`} className="flex flex-col gap-6 px-(--card-spacing) data-ending-style:hidden">
      {children}
    </TabsContent>
  )
}
