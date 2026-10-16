import { HelpKey, HelpKeys, HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { InlineCode } from "@/components/shared/InlineCode"
import { modifierKey, shiftKey } from "@/lib/platform"

/** What makes a commit unassigned, and the keys of the selection (they work while the list has the focus), on hover */
export function UnassignedCommitsHelpPopover() {
  return (
    <HelpPopover title="Unassigned Commits" data-testid="unassigned-commits-help-icon">
      <p>Their subjects start with no prefix that groups a commit into a branch:</p>
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
      <HelpSection title="Shortcuts">
        <HelpKeys>
          <HelpKey click>Select a commit</HelpKey>
          <HelpKey keys={[modifierKey]} click>Add or remove one</HelpKey>
          <HelpKey keys={[shiftKey]} click>Select a range</HelpKey>
          <HelpKey keys={[modifierKey, "A"]}>Select all or none</HelpKey>
          <HelpKey keys={["M"]}>Move to…</HelpKey>
          <HelpKey keys={["S"]}>Squash the selected commits…</HelpKey>
          <HelpKey keys={["Esc"]}>Clear the selection</HelpKey>
        </HelpKeys>
      </HelpSection>
    </HelpPopover>
  )
}
