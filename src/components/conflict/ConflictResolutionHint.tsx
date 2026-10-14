import { ChevronRightIcon, LightbulbIcon } from "lucide-react"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"

/** Why a cherry-pick conflicts and how to resolve it; collapsed by default, so the diffs come first. */
export function ConflictResolutionHint() {
  return (
    <Collapsible className="text-sm text-muted-foreground">
      <CollapsibleTrigger className="group flex items-center gap-1.5 font-medium text-foreground/80 hover:text-foreground">
        <ChevronRightIcon className="size-4 transition-transform group-data-[panel-open]:rotate-90" />
        <LightbulbIcon className="size-4 text-warning" />
        How to resolve
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="flex flex-col gap-2 pt-2 pl-9.5">
          <p>Other commits modified the same files first, so this commit cannot be copied to the virtual branch. You can:</p>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-foreground/85 marker:text-muted-foreground">
            <li>include the missing commits in this branch by changing their prefix to match;</li>
            <li>move the conflicting changes to a different commit or branch;</li>
            <li>exclude this commit from the branch by changing its prefix.</li>
          </ul>
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}
