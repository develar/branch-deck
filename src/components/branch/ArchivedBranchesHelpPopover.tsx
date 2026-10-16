import { HelpPopover } from "@/components/shared/HelpPopover"

export function ArchivedBranchesHelpPopover() {
  return (
    <HelpPopover title="Archived Branches" data-testid="archived-branches-help-icon">
      <p>A sync archives a virtual branch when none of your commits group into it anymore.</p>
      {/* `ARCHIVE_RETENTION_DAYS` in crates/branch-integration/src/archive.rs */}
      <p>A merged one is deleted by the first sync 7 days after it was archived; the others stay until you delete them.</p>
    </HelpPopover>
  )
}
