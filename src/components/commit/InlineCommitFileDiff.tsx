import { useEffect } from "react"
import { Spinner } from "@/components/ui/spinner"
import { LazyCommitFileDiffView } from "@/components/diff/LazyDiffViews"
import { Callout } from "@/components/shared/Callout"
import type { CommitFile } from "@/lib/bindings"
import { loadCommitPatch, useCommitPatch } from "@/stores/commit-patch"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface InlineCommitFileDiffProps {
  commitId: string
  file: CommitFile
  diffViewMode: ConflictDiffViewMode
}

/**
 * The diff of a file under its row in the files of a commit. The first expanded file reads the patch of the whole
 * commit (one `diff-tree -p`), the others of the commit reuse it.
 */
export function InlineCommitFileDiff({ commitId, file, diffViewMode }: InlineCommitFileDiffProps) {
  // a binary file and a file without changed lines (a rename, a mode change) need no patch
  const needsPatch = !file.binary && file.additions + file.deletions > 0
  const entry = useCommitPatch(commitId)

  useEffect(() => {
    if (needsPatch) {
      loadCommitPatch(commitId)
    }
  }, [commitId, needsPatch])

  if (!needsPatch) {
    return <p className="p-4 text-sm text-muted-foreground">{file.binary ? "Binary file" : "No changed lines"}</p>
  }
  if (entry?.kind === "error") {
    return (
      <div className="p-4">
        <Callout tone="destructive" title="Cannot read the changes of the commit">
          {entry.message}
        </Callout>
      </div>
    )
  }
  if (entry?.kind !== "loaded") {
    return (
      <p className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading diff…
      </p>
    )
  }
  return <LazyCommitFileDiffView patch={entry.patch} file={file} diffViewMode={diffViewMode} />
}
