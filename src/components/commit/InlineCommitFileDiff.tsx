import { SubjectFileDiff } from "@/components/conflict/SubjectFileDiffList"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { CommitFile } from "@/lib/bindings"
import { fileDiffOf } from "@/stores/file-diffs"
import type { FileDiffsEntry } from "@/stores/file-diffs"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

export interface InlineCommitFileDiffProps {
  /** The diffs of the files of the commit, read by the tree under the policy */
  diffs: FileDiffsEntry | undefined
  file: CommitFile
  diffViewMode: DiffViewMode
}

/**
 * The diff of a file under its row in the files of a commit: the tree reads the diffs of all files (one
 * `get_file_diffs`), an expanded file reads its two texts by blob id.
 */
export function InlineCommitFileDiff({ diffs, file, diffViewMode }: InlineCommitFileDiffProps) {
  if (diffs?.kind === "error") {
    return (
      <div className="p-4">
        <Callout tone="destructive" title="Cannot read the changes of the commit">
          {diffs.message}
        </Callout>
      </div>
    )
  }
  const diff = fileDiffOf(diffs, file.path)
  if (diff === "pending") {
    return <LoadingLine className="p-4">Loading diff…</LoadingLine>
  }
  return <SubjectFileDiff file={{ path: file.path, status: file.status }} diff={diff} diffViewMode={diffViewMode} />
}
