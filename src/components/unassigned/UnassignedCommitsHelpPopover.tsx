import { Button } from "@/components/ui/button"
import { HelpKey, HelpKeys, HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { InlineCode } from "@/components/shared/InlineCode"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import { modifierKey, shiftKey } from "@/lib/platform"
import { openSettings } from "@/lib/settings-window"
import { useSelectedProjectPath } from "@/stores/repository"

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
      <AgentSubjects />
      <HelpSection title="Shortcuts">
        <HelpKeys>
          <HelpKey click>Select a commit</HelpKey>
          <HelpKey keys={[modifierKey]} click>
            Add or remove one
          </HelpKey>
          <HelpKey keys={[shiftKey]} click>
            Select a range
          </HelpKey>
          <HelpKey keys={[modifierKey, "A"]}>Select all or none</HelpKey>
          <HelpKey keys={["M"]}>Move to…</HelpKey>
          <HelpKey keys={["S"]}>Squash the selected commits…</HelpKey>
          <HelpKey keys={["Esc"]}>Clear the selection</HelpKey>
        </HelpKeys>
      </HelpSection>
    </HelpPopover>
  )
}

/**
 * Whether coding agents write the prefix when they commit here (`branchdeck.agentSubjectBranch`, off by default), read
 * when the popover opens, with the setting one click away
 */
function AgentSubjects() {
  const path = useSelectedProjectPath() ?? ""
  const { state } = useRepositoryConfig(path)
  if (state.status !== "loaded") {
    return null
  }
  return (
    <div className="flex flex-col items-start gap-1.5" data-testid="unassigned-commits-agent-subjects">
      <p>
        {state.config.agentSubjectBranch.value
          ? "Coding agents write (name) when they commit here."
          : "Coding agents write (name) when they commit once Name the branch in each commit subject is on for this repository."}
      </p>
      <Button size="xs" variant="outline" onClick={() => openSettings({ page: "agents" })}>
        Open Agent Settings
      </Button>
    </div>
  )
}
