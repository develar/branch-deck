import { useState } from "react"
import { Button } from "@/components/ui/button"
import { InlineCode } from "@/components/shared/InlineCode"
import { addCompanion, removeCompanion } from "@/hooks/useConfigWrite"
import type { RepositoryConfigRead } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfig } from "@/lib/bindings"
import { repositoryName } from "@/lib/repository-config"

/**
 * "Companions" (openspec/specs/branch/declared-needs/spec.md#companion-repositories): the repositories whose branches a need of
 * this one names as `companion:branch` (`brd need`), each with its path and why it can not be read; Add Companion… picks
 * one with the folder dialog, named by its directory, Remove takes it back, both sides of both repositories as
 * `brd companion` writes them. A companion lists its primary instead: companions are added there
 */
export function CompanionsSetting({ path, config, showWritten }: { path: string; config: RepositoryConfig; showWritten: RepositoryConfigRead["showWritten"] }) {
  const [writing, setWriting] = useState(false)
  const run = (write: () => Promise<boolean>) => {
    setWriting(true)
    void write().finally(() => setWriting(false))
  }
  if (config.primary !== null) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="companions-primary">
        {`A companion of ${repositoryName(config.primary)}: a need here names its branches as ${repositoryName(config.primary)}:branch. Add companions there.`}
      </p>
    )
  }
  return (
    <div className="flex flex-col gap-2">
      {config.companions.length === 0 ? (
        <p className="text-sm text-muted-foreground" data-testid="companions-empty">
          No companion repositories. A need can name a branch of another repository once you add one here.
        </p>
      ) : (
        <ul className="flex flex-col gap-1" aria-label="Companion repositories">
          {config.companions.map(companion => (
            <li key={companion.name} className="flex items-center gap-2" data-testid="companion-row">
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="text-sm font-medium">{companion.name}</span>
                <span className="truncate text-xs text-muted-foreground" title={companion.path}>
                  {companion.path}
                </span>
                {companion.problem !== null && <span className="text-xs text-warning">{`Not readable: ${companion.problem}`}</span>}
              </div>
              <Button
                size="xs"
                variant="ghost"
                aria-label={`Remove ${companion.name}`}
                disabled={writing}
                onClick={() => run(() => removeCompanion(path, companion.name, showWritten))}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <div>
        <Button size="xs" variant="outline" disabled={writing || path === ""} onClick={() => run(() => addCompanion(path, showWritten))}>
          Add Companion…
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        {"A need names a branch of one as "}
        <InlineCode>name:branch</InlineCode>
        {", and its start waits until that branch is in master of the companion"}
      </p>
    </div>
  )
}
