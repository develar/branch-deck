import type { PatronusRobot } from "@/lib/bindings"
import type { RepositoryPatronus } from "@/stores/patronus"

/** What the Patronus store has of a repository on GitHub (`acme/air`) once Patronus answered with `robots` (by id) */
export function testPatronusRepository(robots: Readonly<Record<string, PatronusRobot>> = {}): RepositoryPatronus {
  return {
    availability: { status: "available" },
    robots,
    listedFrom: 0,
    fetchedAt: 0,
    isRefreshing: false,
    needsReview: {},
    githubRepository: "acme/air",
    watched: {},
    failing: {},
    landings: {},
    spans: {},
  }
}
