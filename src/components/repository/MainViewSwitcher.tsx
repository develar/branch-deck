import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { kbdLabel } from "@/lib/menu"
import type { MainView } from "@/stores/main-view"
import { MAIN_VIEWS, showMainView, useMainView } from "@/stores/main-view"

/**
 * Branches · Landing in the title bar, after the repository: where the user is (`spec/repository/main-views.spec.md`).
 * One option set, the stock `ToggleGroup`, the shown view pressed; the arrow keys move the focus between the views and
 * Enter or Space shows the focused one. The names give way to the icons where the room of the repository in the title
 * row (the container of `ConfigurationHeader`) is narrower than `@md`.
 */
export function MainViewSwitcher() {
  const view = useMainView()
  return (
    <ToggleGroup
      aria-label="View"
      variant="outline"
      size="sm"
      spacing={0}
      value={[view]}
      onValueChange={next => {
        const chosen = next.find(it => it !== view)
        if (chosen !== undefined) {
          showMainView(chosen as MainView)
        }
      }}
      className="shrink-0"
      data-testid="main-view-switcher"
    >
      {MAIN_VIEWS.map(entry => (
        <Tooltip key={entry.view}>
          <TooltipTrigger render={<ToggleGroupItem value={entry.view} aria-label={entry.label} data-testid={`main-view-${entry.view}`} />}>
            <entry.icon data-icon="inline-start" />
            <span className="hidden @md:inline">{entry.label}</span>
          </TooltipTrigger>
          <TooltipContent>
            <span className="flex items-center gap-1.5">
              {entry.label}
              {entry.kbds !== undefined && (
                <KbdGroup>
                  {entry.kbds.map(kbd => (
                    <Kbd key={kbd}>{kbdLabel(kbd)}</Kbd>
                  ))}
                </KbdGroup>
              )}
            </span>
          </TooltipContent>
        </Tooltip>
      ))}
    </ToggleGroup>
  )
}
