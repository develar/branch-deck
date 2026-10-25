import { ChevronRightIcon, ExternalLinkIcon, SparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { PopoverTrigger } from "@/components/ui/popover"
import { HelpPopoverContent, HelpSection } from "@/components/shared/HelpPopover"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { InlineCode } from "@/components/shared/InlineCode"
import { useAIToggle } from "@/hooks/useAIToggle"
import { formatTimestamp } from "@/lib/time"
import type { AIError } from "@/stores/ai-suggestions"

/**
 * AI status icon next to the inline branch creator buttons: a click toggles AI (after an error it turns AI off, so the
 * next click retries), hover explains what AI does and what it sends; the click leaves the hover as it is, which then
 * says the new state (`HoverDescription`).
 */
export function AIStatusIndicator() {
  const { aiMode, aiError, statusIconClassName, toggleAIMode } = useAIToggle()

  return (
    <HoverDescription>
      <PopoverTrigger openOnHover render={<Button variant="ghost" size="icon-xs" aria-label={aiMode === "claude" ? "Disable AI" : "Enable AI"} onClick={toggleAIMode} />}>
        <SparklesIcon className={statusIconClassName} data-testid="ai-status-icon" />
      </PopoverTrigger>
      {aiError ? <AIErrorDetails error={aiError} /> : <AIHelp isEnabled={aiMode === "claude"} />}
    </HoverDescription>
  )
}

function AIErrorDetails({ error }: { error: AIError }) {
  return (
    <HelpPopoverContent title="AI suggestions failed" tone="destructive" description={error.message}>
      <Collapsible>
        <CollapsibleTrigger className="group flex items-center gap-1 rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30">
          <ChevronRightIcon className="size-3.5 transition-transform group-data-[panel-open]:rotate-90" />
          Full details
        </CollapsibleTrigger>
        <CollapsibleContent className="pt-2">
          <pre className="overflow-x-auto rounded-lg bg-muted p-2 text-xs break-words whitespace-pre-wrap">{error.details}</pre>
        </CollapsibleContent>
      </Collapsible>
      <div className="flex items-center justify-between text-muted-foreground">
        <span>{formatTimestamp(Math.floor(error.timestamp / 1000))}</span>
        <a href="https://github.com/develar/branch-deck/issues" target="_blank" rel="noreferrer" className="flex items-center gap-1 text-primary hover:underline">
          <ExternalLinkIcon className="size-3" />
          Report issue
        </a>
      </div>
      <p className="border-t border-border pt-3 text-muted-foreground">The button turns AI off; turn it on again to retry.</p>
    </HelpPopoverContent>
  )
}

function AIHelp({ isEnabled }: { isEnabled: boolean }) {
  return (
    <HelpPopoverContent title={isEnabled ? "AI suggestions are on" : "AI suggestions are off"} description={isEnabled ? "The button turns them off." : "The button turns them on."}>
      <p>
        {"Claude suggests branch names from the commit messages and the names of the changed files, as "}
        <InlineCode>git log --name-status</InlineCode>
        {" shows them."}
      </p>
      <HelpSection title="Privacy">
        <p>
          {"Runs "}
          <InlineCode>claude -p --model haiku</InlineCode>
          {" with your Claude Code login: the commit messages and changed file names go to Anthropic. No diffs or file contents are sent."}
        </p>
      </HelpSection>
    </HelpPopoverContent>
  )
}
