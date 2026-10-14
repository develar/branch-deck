import { HelpPopover, HelpSection } from "@/components/shared/HelpPopover"

export function ArchivedBranchesHelpPopover() {
  return (
    <HelpPopover label="About archived branches" data-testid="archived-branches-help-icon">
      <HelpSection title="Automatic archiving">
        <p>Virtual branches are automatically archived when they become inactive (no longer match commits in your working directory).</p>
      </HelpSection>
      <HelpSection title="Automatic cleanup">
        <p>
          {"Merged archived branches are automatically deleted after "}
          <strong className="font-medium">7 days</strong>
          {" to keep your repository clean. Branches that are not merged stay until you delete them."}
        </p>
        <p className="text-muted-foreground">This cleanup happens during branch synchronization.</p>
      </HelpSection>
    </HelpPopover>
  )
}
