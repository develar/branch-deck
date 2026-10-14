import { CircleCheckIcon, CircleXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import type { DownloadProgress } from "@/lib/bindings"
import { cancelDownload, useModelStore } from "@/stores/model"

export interface ModelDownloadProgressProps {
  /**
   * `toast`: body of the live download toast, the one progress surface (no padding);
   * `popover`: hover popover of the downloading icon, one line of state and "Pause"
   */
  variant: "toast" | "popover"
}

/** Live model download progress; subscribes to the model store (`Progress` updates are throttled there). */
export function ModelDownloadProgress({ variant }: ModelDownloadProgressProps) {
  const downloadProgress = useModelStore(state => state.lastProgressEvent)
  if (variant === "popover") {
    return (
      <div className="flex w-80 items-center gap-3 p-3 text-xs" data-testid="model-download-progress-popover">
        <p className="min-w-0 flex-1 truncate tabular-nums">{describeDownload(downloadProgress)}</p>
        <Button size="xs" variant="outline" onClick={() => void cancelDownload()}>
          Pause
        </Button>
      </div>
    )
  }

  return (
    <div className="flex w-full flex-col gap-2 text-xs tabular-nums" data-testid="model-download-progress-toast">
      {downloadProgress
        ? <DownloadProgressContent event={downloadProgress} />
        : <p className="text-muted-foreground">Starting the download…</p>}
    </div>
  )
}

/** One line of state for the popover: the toast shows the details */
function describeDownload(event: DownloadProgress | null): string {
  switch (event?.type) {
    case undefined:
      return "Starting the download…"
    case "Started":
      return `Preparing ${formatFileCount(event.data.totalFiles)}…`
    case "FileStarted":
      return `Downloading ${event.data.fileName}…`
    case "Progress":
      return `Downloading ${event.data.fileName}: ${getPercentage(event.data) ?? 0}%`
    case "FileCompleted":
      return `Downloaded ${event.data.fileName}`
    case "Completed":
      return "Model downloaded"
    case "Cancelled":
      return "Download canceled"
    case "Error":
      return `Download failed: ${event.data.message}`
  }
}

function DownloadProgressContent({ event }: { event: DownloadProgress }) {
  switch (event.type) {
    case "Started":
    case "FileStarted":
    case "Progress": {
      const percentage = event.type === "Progress" ? getPercentage(event.data) : null
      return (
        <div className="flex flex-col gap-2">
          {event.type === "Started"
            ? <p className="text-muted-foreground">{`Preparing ${formatFileCount(event.data.totalFiles)}…`}</p>
            : <p className="font-medium">{event.data.fileName}</p>}

          <div className="flex flex-col gap-1">
            {event.type === "FileStarted" && event.data.fileSize
              ? <p className="text-muted-foreground">{`File size: ${formatBytes(event.data.fileSize)}`}</p>
              : event.type === "Progress" && (
                <div className="flex justify-between text-muted-foreground">
                  <span>{`${formatBytes(event.data.downloaded)} / ${formatBytes(event.data.total)}`}</span>
                  <span>{`${percentage ?? 0}%`}</span>
                </div>
              )}
            {/* indeterminate until the first progress event of a file */}
            <Progress value={percentage} />
          </div>

          {event.type === "Progress" && (
            <div className="flex min-h-4 gap-3 text-muted-foreground">
              {!!event.data.bytesPerSecond && <span>{`${formatBytes(event.data.bytesPerSecond)}/s`}</span>}
              {event.data.secondsRemaining != null && (
                <span>{event.data.secondsRemaining === 0 ? "Finishing…" : `${formatTime(event.data.secondsRemaining)} remaining`}</span>
              )}
            </div>
          )}
        </div>
      )
    }

    case "FileCompleted":
      return (
        <p className="flex items-center gap-1.5 text-success">
          <CircleCheckIcon className="size-3.5 shrink-0" />
          {`Downloaded ${event.data.fileName}`}
        </p>
      )

    case "Completed":
      return (
        <p className="flex items-center gap-1.5 font-medium text-success">
          <CircleCheckIcon className="size-3.5 shrink-0" />
          Model downloaded
        </p>
      )

    case "Error":
      return (
        <div className="flex flex-col gap-2">
          <p className="flex items-center gap-1.5 font-medium text-destructive">
            <CircleXIcon className="size-3.5 shrink-0" />
            Download failed
          </p>
          <p className="text-muted-foreground">{event.data.message}</p>
        </div>
      )

    case "Cancelled":
      return <p className="text-muted-foreground">Download canceled</p>
  }
}

function getPercentage({ downloaded, total }: { downloaded: number, total: number }): number | null {
  return total > 0 ? Math.round((downloaded / total) * 100) : null
}

function formatFileCount(count: number): string {
  return count === 1 ? "1 file" : `${count} files`
}

function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024)
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${(bytes / 1024).toFixed(1)} KB`
}

/** "45s", "2m 5s", "3m" */
function formatTime(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`
  }
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}
