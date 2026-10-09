import { InfoIcon } from "lucide-react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"

export function ArchivedBranchesHelpPopover() {
  return (
    <Popover>
      {/* rendered inside the card header trigger button, so not a button itself */}
      <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
        <InfoIcon
          className="size-3 cursor-help text-muted-foreground transition-colors hover:text-foreground"
          data-testid="archived-branches-help-icon"
        />
      </PopoverTrigger>
      <PopoverContent className="w-auto max-w-sm gap-0 divide-y divide-border p-3 text-left text-xs">
        <div className="flex flex-col gap-1.5 pb-3">
          <div className="font-medium text-muted-foreground">Automatic archiving</div>
          <p>Virtual branches are automatically archived when they become inactive (no longer match commits in your working directory).</p>
        </div>
        <div className="flex flex-col gap-1.5 pt-3">
          <div className="font-medium text-muted-foreground">Automatic cleanup</div>
          <p>
            {"Archived branches are automatically deleted after "}
            <strong>7 days</strong>
            {" to keep your repository clean."}
          </p>
          <p className="text-muted-foreground">This cleanup happens during branch synchronization.</p>
        </div>
      </PopoverContent>
    </Popover>
  )
}
