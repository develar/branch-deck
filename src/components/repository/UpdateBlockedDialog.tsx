import { useRef, useState } from "react"
import { ClipboardCopyIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { toast } from "@/components/ui/toast"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { buildAgentPrompt } from "@/lib/agent-prompt"
import type { UpdateCheck } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"
import { lastFetchTime } from "@/lib/update-state"
import { useSelectedProject } from "@/stores/repository"

export interface UpdateBlockedDialogProps {
  /** The conflicting or blocked update; null while closed */
  check: UpdateCheck | null
  onClose: () => void
}

function PathList({ paths, testId }: { paths: readonly string[], testId: string }) {
  return (
    <ul data-testid={testId} className="flex flex-col gap-0.5 font-mono text-xs">
      {paths.map(path => <li key={path} className="truncate">{path}</li>)}
    </ul>
  )
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
          {state.status === "conflict" ? `Your commits conflict with ${upstream}` : `Your uncommitted changes block the update`}
        </DialogTitle>
        <p className="text-xs text-muted-foreground">
          {fetchTime === null ? "Nothing was changed." : `Nothing was changed. ${upstream} was fetched ${formatRelativeTime(fetchTime)}.`}
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef} itemKey={`${shown.head}-${shown.upstreamHead}`} data-testid="update-blocked-body">
        <div className="flex flex-col gap-4 text-sm">
          {state.status === "conflict" && (
            <>
              <div className="flex flex-col gap-1.5">
                <p>
                  {`Rebasing your ${formatCommitCount(state.commits.length)} onto ${upstream} conflicts`}
                  {state.files.length > 0 ? " in:" : "."}
                </p>
                {state.files.length > 0 && <PathList paths={state.files} testId="update-conflict-files" />}
              </div>
              <div className="flex flex-col gap-1.5">
                <p className="text-muted-foreground">Your commits, oldest first:</p>
                <ul data-testid="update-conflict-commits" className="flex flex-col gap-1">
                  {state.commits.map(commit => (
                    <li key={commit.id} className="flex min-w-0 items-baseline gap-2">
                      <span className="shrink-0 font-mono text-xs text-muted-foreground">{formatShortHash(commit.id)}</span>
                      <span className="min-w-0 truncate">
                        <CommitSubject subject={commit.subject} clamp={false} />
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
              <p className="text-muted-foreground">
                Rebase in a terminal or your IDE, or hand it to a coding agent: the prompt names the commits, the files and
                the rules (keep every message and its prefix, don't push).
              </p>
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
          <ClipboardCopyIcon data-icon="inline-start" />
          Copy Prompt for Agent
        </Button>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
