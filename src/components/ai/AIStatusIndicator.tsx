import { ExternalLinkIcon, ShieldCheckIcon, SparklesIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { ModelDownloadProgress } from "@/components/ai/ModelDownloadProgress"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { useAIToggle } from "@/hooks/useAIToggle"
import type { AIError } from "@/stores/model"
import { useModelStore } from "@/stores/model"

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
        <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
          <SparklesIcon className="size-3.5 animate-pulse text-primary" data-testid="ai-status-icon-downloading" />
        </PopoverTrigger>
        <PopoverContent className="w-auto p-0">
          <ModelDownloadProgress variant="popover" />
        </PopoverContent>
      </Popover>
    )
  }

  return (
    <Popover>
      <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
        <SparklesIcon
          className={cn("size-3.5 cursor-pointer transition-all", statusIconClassName)}
          data-testid="ai-status-icon"
          onClick={toggleAIMode}
        />
      </PopoverTrigger>
      <PopoverContent className="w-xs gap-2 p-3 text-xs">
        {aiError ? <AIErrorDetails error={aiError} /> : <AIHelp isEnabled={aiMode === "enabled"} />}
      </PopoverContent>
    </Popover>
  )
}

function AIErrorDetails({ error }: { error: AIError }) {
  return (
    <>
      <div className="flex items-center gap-2">
        <TriangleAlertIcon className="size-4 text-destructive" />
        <p className="font-semibold text-destructive">AI Malfunction</p>
      </div>
      <div className="flex flex-col gap-2">
        <div className="flex flex-col gap-1 rounded-md border border-destructive/20 bg-destructive/10 p-2">
          <p className="font-medium text-destructive">Error:</p>
          <p className="break-words text-muted-foreground">{error.message}</p>
        </div>
        <details className="cursor-pointer">
          <summary className="text-muted-foreground hover:text-foreground">View full details</summary>
          <pre className="mt-2 overflow-x-auto rounded bg-muted/50 p-2 text-xs break-words whitespace-pre-wrap">{error.details}</pre>
        </details>
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>{new Date(error.timestamp).toLocaleTimeString()}</span>
          <a
            href="https://github.com/develar/branch-deck/issues"
            target="_blank"
            rel="noreferrer"
            className="flex items-center gap-1 text-primary hover:underline"
          >
            <ExternalLinkIcon className="size-2.5" />
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

function AIHelp({ isEnabled }: { isEnabled: boolean }) {
  return (
    <>
      <p className="font-semibold text-foreground">
        AI
        {" "}
        {isEnabled ? "Enabled" : "Disabled"}
        <span className="ml-1 font-normal text-muted-foreground">
          (click to
          {isEnabled ? "disable" : "enable"}
          )
        </span>
      </p>
      <p>AI analyzes only commit metadata to suggest branch names:</p>
      <ul className="ml-2 flex list-inside list-disc flex-col gap-1">
        <li>Commit messages (title and body)</li>
        <li>Modified file names and their status</li>
      </ul>
      <p>
        This is equivalent to:
        {" "}
        <code className="rounded bg-muted/50 px-1 py-0.5">git log --name-status</code>
      </p>
      <div className="flex flex-col gap-1 border-t pt-2">
        <p className="flex items-center gap-1 text-success">
          <ShieldCheckIcon className="size-3" />
          <span className="font-medium">100% Local & Private</span>
        </p>
        <p className="text-muted-foreground">
          Uses the
          {" "}
          <a href="https://huggingface.co/Qwen/Qwen3-1.7B-GGUF" target="_blank" rel="noreferrer" className="text-primary hover:underline">Qwen3-1.7B</a>
          {" "}
          model running entirely on your machine. No data is sent to any external service.
        </p>
      </div>
    </>
  )
}
