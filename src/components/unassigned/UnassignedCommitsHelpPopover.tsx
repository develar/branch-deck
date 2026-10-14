import { Kbd } from "@/components/ui/kbd"
import { HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { InlineCode } from "@/components/shared/InlineCode"
import { modifierKey } from "@/lib/platform"

/** Selection shortcuts and an explanation of unassigned commits, on hover. */
export function UnassignedCommitsHelpPopover() {
  return (
    <HelpPopover label="About unassigned commits" data-testid="unassigned-commits-help-icon">
      <HelpSection title="Selection shortcuts">
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
      </HelpSection>

      <HelpSection title="About unassigned commits">
        <p>These commits won't be assigned to virtual branches because they don't match Branch Deck's grouping patterns:</p>
        <HelpList>
          <li>
            <InlineCode>(feature-name)</InlineCode>
            {" - Explicit prefix"}
          </li>
          <li>
            <InlineCode>ABC-123</InlineCode>
            {" - Issue numbers"}
          </li>
        </HelpList>
      </HelpSection>
    </HelpPopover>
  )
}
