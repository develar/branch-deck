import { useEffect, useEffectEvent, useRef, useState } from "react"
import type { ReactNode } from "react"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AppearancePage } from "@/components/settings/AppearancePage"
import { GeneralPage } from "@/components/settings/GeneralPage"
import { GitHubPage } from "@/components/settings/GitHubPage"
import { NotificationsPage } from "@/components/settings/NotificationsPage"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { SettingsPageId } from "@/lib/settings-pages"
import { resolveSettingsPage, shownSettingsPages } from "@/lib/settings-window"
import type { SettingsTarget } from "@/lib/settings-window"
import { appSettingsStore, setSettingsPage } from "@/stores/settings/appSettings"

const PAGE_LABELS: Record<SettingsPageId, string> = {
  general: "General",
  appearance: "Appearance",
  notifications: "Notifications",
  github: "GitHub",
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
    const field = target?.focus === undefined ? null : document.querySelector<HTMLElement>(`[data-settings-focus="${CSS.escape(target.focus)}"]`)
    if (field !== null) {
      field.focus()
      return () => {}
    }
    const list = listRef.current
    const tab = list?.querySelector<HTMLElement>("[role=tab][aria-selected=true]")
    return list === null || tab === null || tab === undefined ? () => {} : focusWhenTabbable(list, tab)
  })
  useEffect(() => focusOpened(), [openCount])

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
          {shownSettingsPages().map(id => (
            <TabsTrigger key={id} value={id} data-testid={`settings-tab-${id}`}>
              {PAGE_LABELS[id]}
            </TabsTrigger>
          ))}
        </TabsList>
        {/* one island; it scrolls only when a large zoom leaves the page no room */}
        <Card className="min-h-0 flex-1 overflow-y-auto" data-testid="settings-island">
          <SettingsPanel id="general">
            <GeneralPage />
          </SettingsPanel>
          <SettingsPanel id="appearance">
            <AppearancePage />
          </SettingsPanel>
          <SettingsPanel id="notifications">
            <NotificationsPage openCount={openCount} />
          </SettingsPanel>
          <SettingsPanel id="github">
            <GitHubPage openCount={openCount} />
          </SettingsPanel>
        </Card>
      </Tabs>
    </WindowPage>
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

/** A page in the island; the page left stays mounted while Base UI ends its transition, hidden, so the two never stack */
function SettingsPanel({ id, children }: { id: SettingsPageId; children: ReactNode }) {
  return (
    <TabsContent value={id} data-testid={`settings-page-${id}`} className="flex flex-col gap-6 px-(--card-spacing) data-ending-style:hidden">
      {children}
    </TabsContent>
  )
}
