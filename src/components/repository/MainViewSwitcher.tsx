import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { softPrimaryClassName } from "@/components/shared/ChoiceChip"
import { kbdLabel } from "@/lib/menu"
import { MAIN_VIEWS, showMainView, useMainView } from "@/stores/main-view"

/**
 * Branches · Landing in the title bar, after the repository: where the user is (`spec/repository/main-views.spec.md`).
 * A tab list, the arrow keys move between the views, the shown one soft primary (a tint, no surface on the canvas); the
 * names give way to the icons on a narrow window.
 */
export function MainViewSwitcher() {
  const view = useMainView()
  const select = (index: number) => {
    const entry = MAIN_VIEWS[(index + MAIN_VIEWS.length) % MAIN_VIEWS.length]
    if (entry === undefined) {
      return
    }
    showMainView(entry.view)
    document.getElementById(`main-view-tab-${entry.view}`)?.focus()
  }
  return (
    <div role="tablist" aria-label="View" className="flex shrink-0 items-center gap-0.5" data-testid="main-view-switcher">
      {MAIN_VIEWS.map((entry, index) => {
        const selected = entry.view === view
        return (
          <Tooltip key={entry.view}>
            <TooltipTrigger
              render={
                <Button
                  id={`main-view-tab-${entry.view}`}
                  role="tab"
                  aria-selected={selected}
                  aria-label={entry.label}
                  tabIndex={selected ? 0 : -1}
                  variant="ghost"
                  size="xs"
                  className={cn("px-2", selected ? softPrimaryClassName : "text-muted-foreground")}
                  data-testid={`main-view-${entry.view}`}
                  onClick={() => showMainView(entry.view)}
                  onKeyDown={event => {
                    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                      event.preventDefault()
                      select(index + (event.key === "ArrowRight" ? 1 : -1))
                    }
                  }}
                />
              }
            >
              <entry.icon data-icon="inline-start" />
              <span className="hidden @4xl:inline">{entry.label}</span>
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
        )
      })}
    </div>
  )
}
