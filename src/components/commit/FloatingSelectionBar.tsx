import { useEffect, useState } from "react"
import { Popover as PopoverPrimitive } from "@base-ui/react/popover"
import { GitBranchIcon } from "lucide-react"
import { Button } from "@/components/ui/button"

/** Delay before showing, so the bar does not flicker while the selection changes */
const showDelayMs = 150

export interface FloatingSelectionBarProps {
  selectedCount: number
  /** Element the bar floats above (the first selected row) */
  targetElement: HTMLElement | null
  isInlineCreationActive?: boolean
  onCreateBranch: () => void
}

/**
 * "Group into Branch" button floating above the first selected row.
 * Shown 150 ms after the conditions become true, hidden immediately. Non-modal and never takes the focus.
 */
export function FloatingSelectionBar({ selectedCount, targetElement, isInlineCreationActive = false, onCreateBranch }: FloatingSelectionBarProps) {
  const shouldShow = selectedCount > 0 && !isInlineCreationActive && targetElement !== null
  const [delayElapsed, setDelayElapsed] = useState(false)

  useEffect(() => {
    if (!shouldShow) {
      return
    }
    const timer = setTimeout(() => setDelayElapsed(true), showDelayMs)
    return () => {
      clearTimeout(timer)
      setDelayElapsed(false)
    }
  }, [shouldShow])

  const open = shouldShow && delayElapsed

  // controlled without `onOpenChange`: outside presses and Escape (handled by the commit list) do not close it
  return (
    <PopoverPrimitive.Root open={open}>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Positioner
          anchor={targetElement}
          side="top"
          align="center"
          sideOffset={8}
          collisionPadding={8}
          className="isolate z-50"
        >
          <PopoverPrimitive.Popup initialFocus={false} finalFocus={false} className="outline-none">
            <Button
              size="xs"
              variant="outline"
              className="border-primary/50 text-primary shadow-sm transition-shadow hover:bg-primary/10 hover:text-primary hover:shadow-md"
              data-testid="floating-selection-bar"
              onClick={onCreateBranch}
            >
              <GitBranchIcon data-icon="inline-start" />
              Group into Branch
            </Button>
          </PopoverPrimitive.Popup>
        </PopoverPrimitive.Positioner>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  )
}
