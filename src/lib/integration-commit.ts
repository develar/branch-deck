import type { UpstreamCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"

/**
 * "Merged as abc12345 by Safe Push Robot": the baseline commit that brought an archived branch in. No time: the
 * "Integrated" column of the row shows it.
 */
export function describeIntegrationCommit(commit: UpstreamCommit): string {
  return `Merged as ${formatShortHash(commit.id)} by ${commit.committer}`
}
