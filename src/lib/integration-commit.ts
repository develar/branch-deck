import type { UpstreamCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"

/** "Merged as abc12345 by Safe Push Robot, 2 hours ago": the baseline commit that brought an archived branch in */
export function describeIntegrationCommit(commit: UpstreamCommit, now: number = Date.now()): string {
  return `Merged as ${formatShortHash(commit.id)} by ${commit.committer}, ${formatRelativeTime(commit.time * 1000, now)}`
}
