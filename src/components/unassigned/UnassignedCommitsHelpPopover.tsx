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
        <p>Their subjects start with neither of the prefixes that group a commit into a branch:</p>
        <HelpList>
          <li>
            {"an explicit prefix: "}
            <InlineCode>(auth) Add login form</InlineCode>
          </li>
          <li>
            {"an issue key: "}
            <InlineCode>ABC-123 Fix login bug</InlineCode>
          </li>
        </HelpList>
      </HelpSection>
    </HelpPopover>
  )
}
