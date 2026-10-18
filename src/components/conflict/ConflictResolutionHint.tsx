import type { ReactNode } from "react"
import { ChevronRightIcon, LightbulbIcon } from "lucide-react"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"

export interface CollapsibleHintProps {
  /** A `size-4` icon after the chevron */
  icon: ReactNode
  title: string
  children: ReactNode
}

/** An explanation collapsed by default under a muted `font-medium` toggle, so the content of the view comes first. */
export function CollapsibleHint({ icon, title, children }: CollapsibleHintProps) {
  return (
    <Collapsible className="text-sm text-muted-foreground">
      <CollapsibleTrigger className="group flex items-center gap-1.5 rounded-sm font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30">
        <ChevronRightIcon className="size-4 transition-transform group-data-[panel-open]:rotate-90" />
        {icon}
        {title}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="flex flex-col gap-2 pt-2 pl-9.5">{children}</div>
      </CollapsibleContent>
    </Collapsible>
  )
}

/** Why a commit conflicts and how to resolve it */
export function ConflictResolutionHint() {
  return (
    <CollapsibleHint icon={<LightbulbIcon className="size-4 text-warning" />} title="How to resolve">
      <p>Other commits modified the same files first, so this commit cannot be copied to the virtual branch. You can:</p>
      <HintList>
        <li>include the missing commits in this branch by changing their prefix to match;</li>
        <li>move the conflicting changes to a different commit or branch;</li>
        <li>exclude this commit from the branch by changing its prefix.</li>
      </HintList>
    </CollapsibleHint>
  )
}

/** A list of steps in the colour of the text around it (a muted hint, a callout) */
export function HintList({ children }: { children: ReactNode }) {
  return <ul className="flex list-disc flex-col gap-1 pl-5 marker:text-muted-foreground">{children}</ul>
}
