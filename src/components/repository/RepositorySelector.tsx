import { useEffect, useMemo, useState } from "react"
import { FolderSearchIcon } from "lucide-react"
import { cn } from "cn"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { logError } from "@/lib/errors"
import { formatRelativeTime } from "@/lib/time"
import { browseRepository, selectProject, useRepository } from "@/stores/repository"
import { useRecentProjects } from "@/stores/settings/repositorySettings"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"

interface RepositorySelectorProps {
  disabled?: boolean
}

interface ProjectItem {
  label: string
  value: string
  error?: string | null
  project: ProjectMetadata
}

export function RepositorySelector({ disabled }: RepositorySelectorProps) {
  const pathValidation = useRepository(state => state.pathValidation)
  const recentProjects = useRecentProjects(projects => projects)
  const homeDir = useHomeDir()

  const items = useMemo<ProjectItem[]>(() => {
    const result: ProjectItem[] = recentProjects.map(project => ({
      label: formatPathWithTruncation(project.path, homeDir),
      value: project.path,
      error: undefined,
      project,
    }))

    // an invalid path is not added to recent projects, but it is shown as the current value together with its error
    if (!pathValidation.valid && pathValidation.path) {
      const invalidPath = pathValidation.path
      result.unshift({
        label: formatPathWithTruncation(invalidPath, homeDir),
        value: invalidPath,
        // the message of the error, under the path
        error: pathValidation.errorDetails ?? pathValidation.error,
        project: { path: invalidPath, cachedBranchPrefix: undefined },
      })
    }
    return result
  }, [recentProjects, pathValidation, homeDir])

  const value = !pathValidation.valid && pathValidation.path
    ? pathValidation.path
    : (recentProjects[0]?.path ?? null)

  const onValueChange = (newValue: string | null) => {
    if (!newValue) {
      console.error("Selected project is null")
      return
    }
    // the path is validated explicitly on browse; an invalid saved path is reported on prefix load or sync
    const project = items.find(item => item.value === newValue)?.project ?? { path: newValue, cachedBranchPrefix: undefined }
    selectProject(project)
  }

  const hasError = !!pathValidation.error
  return (
    // the selected path explains itself: the tooltip only invites to choose one or says why it fails
    <Tooltip disabled={value !== null && !hasError}>
      {/* gives way first on a narrow window */}
      <TooltipTrigger render={<FieldGroup className="min-w-0" />}>
        <Select items={items} value={value} onValueChange={onValueChange} disabled={disabled}>
          <SelectTrigger size="sm" className="w-3xs min-w-0 border-border bg-background" aria-invalid={hasError || undefined}>
            <SelectValue>
              {(selected: string | null) => (
                // rtl clips the start, the project name stays; bdi keeps the path itself left to right
                <span dir="rtl" className="truncate"><bdi>{items.find(item => item.value === selected)?.label ?? "Select repository…"}</bdi></span>
              )}
            </SelectValue>
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false} className="w-auto min-w-(--anchor-width)">
            {items.map(item => (
              <SelectItem key={item.value} value={item.value}>
                <div className="flex flex-col">
                  <div className="whitespace-nowrap">{item.label}</div>
                  {item.error
                    ? <div className="truncate text-xs text-destructive">{item.error}</div>
                    : item.project.lastSyncTime
                      ? (
                          <div className="text-xs whitespace-nowrap text-muted-foreground">
                            {item.project.lastBranchCount === 1 ? "1 branch" : `${item.project.lastBranchCount || 0} branches`}
                            {` · ${formatRelativeTime(item.project.lastSyncTime)}`}
                          </div>
                        )
                      : null}
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button
          variant="outline"
          size="icon-sm"
          disabled={disabled}
          aria-label="Browse repository"
          className={cn(hasError && "border-destructive/50 text-destructive hover:bg-destructive/10 hover:text-destructive")}
          data-testid="browse-repository-button"
          onClick={() => void browseRepository()}
        >
          <FolderSearchIcon />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{pathValidation.error || "Select a Git repository to manage branches"}</TooltipContent>
    </Tooltip>
  )
}

function useHomeDir(): string {
  const [homeDir, setHomeDir] = useState("")
  useEffect(() => {
    let cancelled = false
    import("@tauri-apps/api/path")
      .then(({ homeDir }) => homeDir())
      .then((dir) => {
        if (!cancelled) {
          setHomeDir(dir)
        }
      })
      .catch((error: unknown) => {
        logError("Failed to resolve home directory", error)
      })
    return () => {
      cancelled = true
    }
  }, [])
  return homeDir
}

/** Replaces the home directory with `~`. */
function formatPath(path: string, homeDir: string): string {
  if (homeDir && path.startsWith(homeDir)) {
    return path.replace(homeDir, "~")
  }
  return path
}

/** Shortens a path for the dropdown, keeping the project name and as many trailing segments as fit. */
function formatPathWithTruncation(path: string, homeDir: string, maxLength: number = 36): string {
  const formatted = formatPath(path, homeDir)
  if (formatted.length <= maxLength) {
    return formatted
  }

  const segments = formatted.split("/").filter(it => it.length > 0)
  if (segments.length === 0) {
    return formatted
  }

  const projectName = segments[segments.length - 1]!
  // if the project name alone is too long, truncate it
  if (projectName.length > maxLength - 1) {
    return "…" + projectName.slice(-(maxLength - 1))
  }

  // build the path from the end, keeping as much as possible
  let result = projectName
  let i = segments.length - 2
  while (i >= 0) {
    const newSegment = segments[i]! + "/"
    // reserve space for "…/" at the beginning
    if (result.length + newSegment.length + 2 > maxLength) {
      const remainingSpace = maxLength - result.length - 2
      if (remainingSpace > 0 && i === 0 && segments[0]!.length <= remainingSpace) {
        // the first segment fits
        result = segments[0]! + "/…/" + result
      }
      else {
        result = "…/" + result
      }
      break
    }
    result = newSegment + result
    i--
  }
  return result
}
