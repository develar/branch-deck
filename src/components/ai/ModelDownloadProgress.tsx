import { TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import type { DownloadProgress } from "@/lib/bindings"
import { cancelDownload, useModelStore } from "@/stores/model"

export interface ModelDownloadProgressProps {
  /** `toast`: body of the live download toast (no padding); `popover`: hover popover of the downloading icon, with a pause button */
  variant: "toast" | "popover"
}

/** Live model download progress; subscribes to the model store (`Progress` updates are throttled there). */
export function ModelDownloadProgress({ variant }: ModelDownloadProgressProps) {
  const downloadProgress = useModelStore(state => state.lastProgressEvent)
  const isPopover = variant === "popover"
  return (
    <div
      className={cn("flex flex-col gap-2 text-xs", isPopover ? "w-xs p-3" : "w-full")}
      data-testid={isPopover ? "model-download-progress-popover" : "model-download-progress-toast"}
    >
      {downloadProgress
        ? <DownloadProgressContent event={downloadProgress} showCancelButton={isPopover} />
        : <div className="text-muted-foreground">Initializing download…</div>}
    </div>
  )
}

function DownloadProgressContent({ event, showCancelButton }: { event: DownloadProgress, showCancelButton: boolean }) {
  switch (event.type) {
    case "Started":
    case "FileStarted":
    case "Progress": {
      const percentage = event.type === "Progress" && event.data.total > 0
        ? Math.round((event.data.downloaded / event.data.total) * 100)
        : null
      return (
        <div className="flex flex-col gap-2">
          {event.type === "Started"
            ? (
                <p className="text-muted-foreground">
                  Preparing
                  {" "}
                  {event.data.totalFiles}
                  {" "}
                  {event.data.totalFiles === 1 ? "file" : "files"}
                  …
                </p>
              )
            : <p className="font-semibold">{event.data.fileName}</p>}

          <div className="flex flex-col gap-1">
            {event.type === "FileStarted" && event.data.fileSize
              ? (
                  <p className="text-muted-foreground">
                    File size:
                    {formatBytes(event.data.fileSize)}
                  </p>
                )
              : event.type === "Progress" && (
                <div className="flex justify-between text-muted-foreground">
                  <span>
                    {formatBytes(event.data.downloaded)}
                    {" "}
                    /
                    {" "}
                    {formatBytes(event.data.total)}
                  </span>
                  <span>
                    {percentage ?? 0}
                    %
                  </span>
                </div>
              )}
            {/* indeterminate until the first progress event of a file */}
            <Progress value={percentage} />
          </div>

          {event.type === "Progress" && (
            <div className="flex min-h-4 gap-3 text-muted-foreground">
              {!!event.data.bytesPerSecond && (
                <span>
                  {formatBytes(event.data.bytesPerSecond)}
                  /s
                </span>
              )}
              {event.data.secondsRemaining != null && (
                <span>
                  {formatTime(event.data.secondsRemaining)}
                  {" "}
                  remaining
                </span>
              )}
            </div>
          )}

          {showCancelButton && (
            <div className="flex justify-end border-t pt-2">
              <Button size="xs" variant="outline" onClick={() => void cancelDownload()}>
                Pause download
              </Button>
            </div>
          )}
        </div>
      )
    }

    case "FileCompleted":
      return (
        <p className="text-success">
          ✓
          {event.data.fileName}
          {" "}
          completed
        </p>
      )

    case "Completed":
      return <p className="font-semibold text-success">All files downloaded</p>

    case "Error":
      return (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <TriangleAlertIcon className="size-4 text-destructive" />
            <p className="font-semibold text-destructive">Download Failed</p>
          </div>
          <p className="text-muted-foreground">{event.data.message}</p>
        </div>
      )

    case "Cancelled":
      return <p className="text-muted-foreground">Download canceled</p>
  }
}

function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024)
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${(bytes / 1024).toFixed(1)} KB`
}

function formatTime(seconds: number): string {
  if (seconds === 0) {
    return "Finishing…"
  }
  if (seconds < 60) {
    return `${seconds}s`
  }
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}
