import { Tabs as TabsPrimitive } from "@base-ui/react/tabs"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { SettingsScope } from "@/lib/settings-window"

/** A tab of the scope: a segment of the control, the selected one raised as the pressed buttons of the app */
const TAB_CLASS =
  "inline-flex h-6 min-w-0 items-center rounded-xl px-2.5 text-sm whitespace-nowrap text-muted-foreground transition-colors hover:text-foreground focus-visible:bd-focus data-active:bg-background data-active:text-foreground data-active:shadow-xs dark:data-active:bg-input/50"

/**
 * The scope tabs of a page of the git config (spec/repository/settings.spec.md#scope): "All repositories", the value
 * every repository uses, in the app and brd, and the selected repository, whose own value a change writes to its
 * `.git/config`; the line under them says where a change goes. Without a repository only the value of every
 * repository is shown, without tabs. A switch of the repository in the main window shows its name with a flash, and
 * says it to a screen reader. Base UI's tabs, not those of `ui/tabs`: their styles follow the vertical sidebar around them
 */
export function ScopeTabs() {
  const { scope, setScope, path, name } = useSettingsScope()
  return (
    <div className="flex flex-col gap-1.5" data-testid="settings-scope">
      {path !== "" && (
        <TabsPrimitive.Root value={scope} onValueChange={value => setScope(value as SettingsScope)}>
          <TabsPrimitive.List aria-label="Scope" activateOnFocus className="inline-flex max-w-full items-center gap-0.5 rounded-2xl bg-muted p-[3px]">
            <TabsPrimitive.Tab value="all" className={TAB_CLASS} data-testid="settings-scope-all">
              All repositories
            </TabsPrimitive.Tab>
            <TabsPrimitive.Tab value="repository" className={TAB_CLASS} data-testid="settings-scope-repository" title={path}>
              <span key={path} className="animate-in truncate duration-700 fade-in">
                {name}
              </span>
            </TabsPrimitive.Tab>
          </TabsPrimitive.List>
        </TabsPrimitive.Root>
      )}
      <p className="text-xs text-muted-foreground" data-testid="settings-scope-line">
        {scope === "all" ? "What every repository uses, in the app and brd" : `Changes here apply only to ${name}, saved in its .git/config`}
      </p>
      <span className="sr-only" aria-live="polite">
        {path === "" ? "" : `The repository is ${name}`}
      </span>
    </div>
  )
}
