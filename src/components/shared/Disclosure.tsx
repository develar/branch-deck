import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"

/**
 * The one disclosure of a document (docs/design/ui-theme.md, "Disclosures"): a chevron before its title, at the size of
 * the text around it, which a click turns and opens the content below, so a disclosure grows downward. A part of a
 * body folded by Rust, a commit that follows a step up, what the overview folds (the API no use breaks, why the uses
 * are unknown). `prefix` leads the title muted ("Follows up"); `tone` mutes the title too where it is a note
 */
export function Disclosure({
  title,
  prefix,
  tone = "default",
  defaultOpen = false,
  testId,
  triggerTestId,
  children,
  className,
}: {
  title: ReactNode
  prefix?: ReactNode
  tone?: "default" | "muted"
  defaultOpen?: boolean
  testId?: string
  triggerTestId?: string
  children: ReactNode
  className?: string
}) {
  return (
    <Collapsible defaultOpen={defaultOpen} className={cn("flex min-w-0 flex-col gap-1.5", className)} data-testid={testId}>
      <CollapsibleTrigger
        data-testid={triggerTestId}
        className={cn(
          "group flex min-w-0 items-baseline gap-1 self-start rounded-sm text-left outline-none hover:underline focus-visible:bd-focus",
          tone === "muted" ? "text-muted-foreground hover:text-foreground" : "text-foreground",
        )}
      >
        <DisclosureChevron />
        {prefix !== undefined && <span className="shrink-0 text-muted-foreground">{prefix}</span>}
        <span className="min-w-0">{title}</span>
      </CollapsibleTrigger>
      <CollapsibleContent className="pl-[1.125rem]">{children}</CollapsibleContent>
    </Collapsible>
  )
}

/** The chevron of a disclosure, turned while open (`group-data-[panel-open]` of a trigger, `aria-expanded` of a button) */
export function DisclosureChevron() {
  return (
    <ChevronRightIcon
      aria-hidden
      className="size-3.5 shrink-0 self-center text-muted-foreground transition-transform group-aria-expanded:rotate-90 group-data-[panel-open]:rotate-90"
    />
  )
}
