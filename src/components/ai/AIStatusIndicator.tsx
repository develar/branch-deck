import { ExternalLinkIcon, ShieldCheckIcon, SparklesIcon } from "lucide-react"
import { useEffect } from "react"
import { ModelDownloadProgress } from "@/components/ai/ModelDownloadProgress"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Callout } from "@/components/shared/Callout"
import { HelpList, HelpPopoverContent } from "@/components/shared/HelpPopover"
import { InlineCode } from "@/components/shared/InlineCode"
import { useAIToggle } from "@/hooks/useAIToggle"
import type { AIError } from "@/stores/model"
import { ensureModelStatus, useModelStore } from "@/stores/model"

/**
 * AI status icon next to the inline branch creator buttons: click toggles AI (or retries after an error),
 * hover explains what AI does; while the model downloads, hover shows the download progress.
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
        render={<Button variant="ghost" size="icon-xs" aria-label={aiError ? "Retry AI" : aiMode === "enabled" ? "Disable AI" : "Enable AI"} onClick={toggleAIMode} />}
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
      <Callout tone="destructive" title="AI Malfunction" className="text-xs">
        <p className="break-words">{error.message}</p>
      </Callout>
      <div className="flex flex-col gap-2">
        <details className="group cursor-pointer">
          <summary className="text-muted-foreground hover:text-foreground">View full details</summary>
          <pre className="overflow-x-auto rounded-lg bg-muted p-2 text-xs break-words whitespace-pre-wrap group-open:mt-2">{error.details}</pre>
        </details>
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>{new Date(error.timestamp).toLocaleTimeString()}</span>
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
      </div>
      <div className="border-t pt-2">
        <p className="text-muted-foreground">Click the icon to retry or disable AI</p>
      </div>
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
      <p className="flex gap-1">
        <span className="font-medium">{isEnabled ? "AI Enabled" : "AI Disabled"}</span>
        <span className="text-muted-foreground">{isEnabled ? "(click to disable)" : "(click to enable)"}</span>
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
      <div className="flex flex-col gap-1 border-t border-border pt-2">
        <p className="flex items-center gap-1 font-medium text-success">
          <ShieldCheckIcon className="size-3" />
          100% Local & Private
        </p>
        <p className="text-muted-foreground">
          Uses
          {" "}
          <ModelLink />
          {" "}
          running entirely on your machine. No data is sent to any external service.
        </p>
      </div>
    </>
  )
}
