import { useRef, useState } from "react"
import { ChevronRightIcon, CopyIcon, TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { toast } from "@/components/ui/toast"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { CopyButton } from "@/components/shared/CopyButton"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { buildAgentPrompt } from "@/lib/agent-prompt"
import type { UpdateCheck, UpdateCommit } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"
import { groupConflictFiles, splitPath } from "@/lib/update-conflict"
import type { ConflictFile } from "@/lib/update-conflict"
import { fileCount, lastFetchTime } from "@/lib/update-state"
import { useSelectedProject } from "@/stores/repository"

export interface UpdateBlockedDialogProps {
  /** The conflicting or blocked update; null while closed */
  check: UpdateCheck | null
  onClose: () => void
}

/** The file name first, then its directory, muted; the directory gives way first, from its start */
function PathLabel({ path }: { path: string }) {
  const { name, directory } = splitPath(path)
  return (
    <span className="group/path flex min-w-0 items-center gap-2 font-mono text-xs">
      <span className="shrink-0 text-foreground">{name}</span>
      {directory !== "" && (
        // rtl clips the start; bdi keeps the path itself left to right
        <span dir="rtl" title={path} className="min-w-0 truncate text-muted-foreground"><bdi>{directory}</bdi></span>
      )}
      <CopyButton text={() => path} tooltip="Copy path" className="-my-1 shrink-0 group-hover/path:opacity-100" />
    </span>
  )
}

function PathList({ paths, testId }: { paths: readonly string[], testId: string }) {
  return (
    <ul data-testid={testId} className="flex flex-col gap-0.5">
      {paths.map(path => <li key={path} data-path={path}><PathLabel path={path} /></li>)}
    </ul>
  )
}

/** The cells of a commit in a row of a subgrid: the hash, the subject, and the author with the time, if asked */
function CommitCells({ commit, showAuthor = false }: { commit: UpdateCommit, showAuthor?: boolean }) {
  return (
    <>
      <span className="font-mono text-xs text-muted-foreground">{formatShortHash(commit.id)}</span>
      <span className="min-w-0 truncate">
        <CommitSubject subject={commit.subject} clamp={false} />
      </span>
      {showAuthor && (
        <span className="text-xs whitespace-nowrap text-muted-foreground">
          {`${commit.author}, ${formatRelativeTime(commit.time * 1000)}`}
        </span>
      )}
    </>
  )
}

/** A file with the commits on both sides that change it: the user's, then the upstream's with their authors */
function ConflictFileItem({ file, upstream }: { file: ConflictFile, upstream: string }) {
  const rows = [
    ...file.local.map((commit, index) => ({ commit, side: "local", label: index === 0 ? "yours" : "" })),
    ...file.upstream.map((commit, index) => ({ commit, side: "upstream", label: index === 0 ? upstream : "" })),
  ]
  return (
    <li data-testid="update-conflict-file" data-path={file.path} className="flex flex-col gap-1">
      <PathLabel path={file.path} />
      {rows.length > 0 && (
        <ul className="grid grid-cols-[max-content_max-content_minmax(0,1fr)_max-content] gap-x-3 gap-y-1 pl-4">
          {rows.map(({ commit, side, label }) => (
            <li key={`${side}-${commit.id}`} data-side={side} className="col-span-full grid grid-cols-subgrid items-baseline">
              <span className="text-xs text-muted-foreground">{label}</span>
              <CommitCells commit={commit} showAuthor={side === "upstream"} />
            </li>
          ))}
        </ul>
      )}
    </li>
  )
}

/** All local commits, oldest first; those that change a likely conflicting file are marked */
function AllCommits({ commits, defaultOpen }: { commits: readonly UpdateCommit[], defaultOpen: boolean }) {
  return (
    <Collapsible defaultOpen={defaultOpen} className="flex flex-col gap-1.5">
      <CollapsibleTrigger className="group flex w-fit items-center gap-1 text-muted-foreground hover:text-foreground">
        <ChevronRightIcon className="size-4 transition-transform group-data-[panel-open]:rotate-90" />
        {`All ${formatCommitCount(commits.length)} to rebase, oldest first`}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ul data-testid="update-conflict-commits" className="grid grid-cols-[auto_max-content_minmax(0,1fr)] gap-x-2 gap-y-1 pl-1">
          {commits.map(commit => (
            <li key={commit.id} className="col-span-full grid grid-cols-subgrid items-baseline">
              {commit.files.length > 0
                ? <TriangleAlertIcon aria-label="Changes a likely conflicting file" className="size-3 self-center text-warning" />
                : <span className="size-3" />}
              <CommitCells commit={commit} />
            </li>
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

function dialogTitle({ state, upstream }: UpdateCheck): string {
  if (state.status !== "conflict") {
    return "Your uncommitted changes block the update"
  }
  if (state.files.length === 0) {
    return `Your commits conflict with ${upstream}`
  }
  return `${fileCount(state.files.length)} ${state.files.length === 1 ? "conflicts" : "conflict"} with ${upstream}`
}

/**
 * Explains why Update Project changed nothing: the local commits conflict with the upstream, or uncommitted changes
 * sit on files the upstream changed. The primary action copies a prompt that hands the update to a coding agent.
 *
 * Keep it mounted while closed (see `PreviewDialogFrame`).
 */
export function UpdateBlockedDialog({ check, onClose }: UpdateBlockedDialogProps) {
  // the last check stays while the closing animation runs
  const [shown, setShown] = useState(check)
  if (check !== null && check !== shown) {
    setShown(check)
  }
  const project = useSelectedProject()
  const bodyRef = useRef<HTMLDivElement>(null)
  if (shown === null) {
    return null
  }

  const { state, upstream } = shown
  const fetchTime = lastFetchTime(shown, project?.lastFetchTime)
  const copyPrompt = async () => {
    const prompt = buildAgentPrompt({ repositoryPath: project?.path ?? "", check: shown, fetchTime })
    try {
      await navigator.clipboard.writeText(prompt)
      toast.add({ type: "success", title: "Prompt copied", description: "Paste it into your coding agent" })
    }
    catch (error) {
      notifyError("Copy Failed", error)
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="update-blocked-dialog"
      open={check !== null}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          onClose()
        }
      }}
      initialFocus={bodyRef}
    >
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">
          {dialogTitle(shown)}
        </DialogTitle>
        <p className="text-xs text-muted-foreground">
          {fetchTime === null ? "Nothing was changed." : `Nothing was changed. ${upstream} was fetched ${formatRelativeTime(fetchTime)}.`}
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef} itemKey={`${shown.head}-${shown.upstreamHead}`} data-testid="update-blocked-body">
        <div className="flex flex-col gap-4 text-sm">
          {state.status === "conflict" && (
            <>
              <p className="text-muted-foreground">Rebase in a terminal or your IDE, or copy a prompt for a coding agent.</p>
              {state.files.length > 0 && (
                <ul data-testid="update-conflict-files" className="flex flex-col gap-3">
                  {groupConflictFiles(state.files, state.commits, state.upstreamCommits).map(file => (
                    <ConflictFileItem key={file.path} file={file} upstream={upstream} />
                  ))}
                </ul>
              )}
              {/* a newly shown conflict starts collapsed again */}
              <AllCommits key={`${shown.head}-${shown.upstreamHead}`} commits={state.commits} defaultOpen={state.files.length === 0} />
            </>
          )}
          {state.status === "blocked" && (
            <>
              <div className="flex flex-col gap-1.5">
                <p>{`${upstream} changes files that have uncommitted changes or are untracked:`}</p>
                <PathList paths={state.paths} testId="update-blocked-paths" />
              </div>
              <p className="text-muted-foreground">
                Commit or stash them, then update again; changes to other files don't block the update and stay as they
                are. Or hand it to a coding agent: the prompt stashes them, updates and restores them.
              </p>
            </>
          )}
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter start={<span>{`${shown.branch} ← ${upstream}`}</span>}>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
        <Button size="xs" data-testid="copy-agent-prompt" onClick={() => void copyPrompt()}>
          <CopyIcon data-icon="inline-start" />
          Copy Prompt for Agent
        </Button>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
