import { ChevronRightIcon, ExternalLinkIcon, SparklesIcon } from "lucide-react"
import { useEffect } from "react"
import { ModelDownloadProgress } from "@/components/ai/ModelDownloadProgress"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Callout } from "@/components/shared/Callout"
import { HelpList, HelpPopoverContent, HelpSection } from "@/components/shared/HelpPopover"
import { InlineCode } from "@/components/shared/InlineCode"
import { useAIToggle } from "@/hooks/useAIToggle"
import { formatTimestamp } from "@/lib/time"
import type { AIError } from "@/stores/model"
import { ensureModelStatus, useModelStore } from "@/stores/model"

/**
 * AI status icon next to the inline branch creator buttons: a click toggles AI (after an error it turns AI off, so the
 * next click retries), hover explains what AI does; while the model downloads, hover shows its state and "Pause" (the
 * download toast shows the progress).
 */
export function AIStatusIndicator() {
  const { aiMode, aiError, statusIconClassName, toggleAIMode } = useAIToggle()
  const isDownloading = useModelStore(state => state.isDownloading)

  if (isDownloading) {
    return (
      <Popover>
        <PopoverTrigger openOnHover render={<Button variant="ghost" size="icon-xs" aria-label="AI model download" />}>
          <SparklesIcon className="animate-pulse text-primary" data-testid="ai-status-icon-downloading" />
        </PopoverTrigger>
        <PopoverContent className="w-auto p-0">
          <ModelDownloadProgress variant="popover" />
        </PopoverContent>
      </Popover>
    )
  }

  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        render={<Button variant="ghost" size="icon-xs" aria-label={aiMode === "enabled" ? "Disable AI" : "Enable AI"} onClick={toggleAIMode} />}
      >
        <SparklesIcon className={statusIconClassName} data-testid="ai-status-icon" />
      </PopoverTrigger>
      <HelpPopoverContent>
        {aiError ? <AIErrorDetails error={aiError} /> : <AIHelp isEnabled={aiMode === "enabled"} />}
      </HelpPopoverContent>
    </Popover>
  )
}

function AIErrorDetails({ error }: { error: AIError }) {
  return (
    <>
      <Callout tone="destructive" title="AI suggestions failed" className="text-xs">
        <p className="break-words">{error.message}</p>
      </Callout>
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
        <a
          href="https://github.com/develar/branch-deck/issues"
          target="_blank"
          rel="noreferrer"
          className="flex items-center gap-1 text-primary hover:underline"
        >
          <ExternalLinkIcon className="size-3" />
          Report issue
        </a>
      </div>
      <p className="border-t border-border pt-2 text-muted-foreground">The button turns AI off; turn it on again to retry.</p>
    </>
  )
}

/** "the <model> model" linked to its Hugging Face page; "a local model" until the backend reports the model */
function ModelLink() {
  const modelStatus = useModelStore(state => state.modelStatus)
  useEffect(ensureModelStatus, [])

  if (modelStatus === null) {
    return <>a local model</>
  }
  return (
    <>
      the
      {" "}
      <a href={modelStatus.modelUrl} target="_blank" rel="noreferrer" className="text-primary hover:underline">{modelStatus.modelName}</a>
      {" "}
      model
    </>
  )
}

function AIHelp({ isEnabled }: { isEnabled: boolean }) {
  return (
    <>
      <p>
        <span className="font-medium">{isEnabled ? "AI suggestions are on." : "AI suggestions are off."}</span>
        {" "}
        <span className="text-muted-foreground">{isEnabled ? "The button turns them off." : "The button turns them on."}</span>
      </p>
      <p>AI analyzes only commit metadata to suggest branch names:</p>
      <HelpList>
        <li>Commit messages (title and body)</li>
        <li>Modified file names and their status</li>
      </HelpList>
      <p>
        {"This is equivalent to: "}
        <InlineCode>git log --name-status</InlineCode>
      </p>
      <HelpSection title="Privacy">
        <p>
          {"Uses "}
          <ModelLink />
          , which runs on your machine. No data is sent to any external service.
        </p>
      </HelpSection>
    </>
  )
}
