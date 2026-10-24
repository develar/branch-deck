import { useEffect, useEffectEvent } from "react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import type { SymbolSearch } from "@/hooks/useSymbolNavigation"

/**
 * A Go to Declaration or Find Usages the backend still answers, at the name (spec/commit/symbol-navigation.spec.md):
 * the index of the repository is waited for as long as it runs, so the search says so, and Escape or Cancel stops it.
 * It fades in late: an answer of the review or of a warm index comes before it shows
 */
export function SymbolSearchStatus({ search, onCancel }: { search: SymbolSearch; onCancel: () => void }) {
  const cancel = useEffectEvent(onCancel)
  useEffect(() => {
    // before the Escape of the view and of the window, which would close it
    const listener = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) {
        event.preventDefault()
        event.stopPropagation()
        cancel()
      }
    }
    window.addEventListener("keydown", listener, true)
    return () => window.removeEventListener("keydown", listener, true)
  }, [])
  const label = search.kind === "usages" ? "Finding usages…" : "Looking for the declaration…"
  return (
    <div
      role="status"
      aria-label={label}
      data-testid="symbol-search"
      className="fixed z-50 flex animate-in items-center gap-2 rounded-lg bg-popover py-1 pr-1 pl-2.5 text-xs text-popover-foreground shadow-lg ring-1 ring-foreground/5 delay-300 duration-100 fade-in-0 fill-mode-backwards"
      style={{ left: search.at.x, top: search.at.y + 4 }}
    >
      <Spinner className="size-3" />
      {label}
      <Button variant="ghost" size="xs" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  )
}
