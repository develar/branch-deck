import { InfoIcon } from "lucide-react"
import { Kbd } from "@/components/ui/kbd"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"

const isMac = navigator.userAgent.toLowerCase().includes("mac")
const modifierKey = isMac ? "⌘" : "Ctrl"

/** Selection shortcuts and an explanation of unassigned commits, on hover. */
export function UnassignedCommitsHelpPopover() {
  return (
    <Popover>
      <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
        <InfoIcon
          className="size-3 cursor-help text-muted-foreground transition-colors hover:text-foreground"
          data-testid="unassigned-commits-help-icon"
        />
      </PopoverTrigger>
      <PopoverContent className="w-auto max-w-sm gap-0 divide-y divide-border p-3 text-xs">
        <div className="flex flex-col gap-1.5 pb-3">
          <div className="font-medium text-muted-foreground">Selection shortcuts</div>
          <div>Click to select a commit</div>
          <div className="flex items-center gap-1">
            <Kbd>{modifierKey}</Kbd>
            <span>+ Click for multiple</span>
          </div>
          <div className="flex items-center gap-1">
            <Kbd>Shift</Kbd>
            <span>+ Click for range</span>
          </div>
          <div className="flex items-center gap-1">
            <Kbd>{modifierKey}</Kbd>
            <Kbd>A</Kbd>
            <span>Select all</span>
          </div>
          <div className="flex items-center gap-1">
            <Kbd>Esc</Kbd>
            <span>Clear selection</span>
          </div>
        </div>

        <div className="flex flex-col gap-1.5 pt-3">
          <div className="font-medium text-muted-foreground">About unassigned commits</div>
          <p>These commits won't be assigned to virtual branches because they don't match Branch Deck's grouping patterns:</p>
          <ul className="flex flex-col gap-1">
            <li className="flex items-start gap-1">
              <span>•</span>
              <span>
                <code className="rounded bg-background px-1 text-[10px]">(feature-name)</code>
                {" - Explicit prefix"}
              </span>
            </li>
            <li className="flex items-start gap-1">
              <span>•</span>
              <span>
                <code className="rounded bg-background px-1 text-[10px]">ABC-123</code>
                {" - Issue numbers"}
              </span>
            </li>
          </ul>
        </div>
      </PopoverContent>
    </Popover>
  )
}
