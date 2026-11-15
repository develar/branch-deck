/** The directory of the builds of the Air page, below the root of the repository (`scripts/air-dist.mjs`) */
export const AIR_BUILDS: string
/** The directory a new build of the page goes into */
export function nextAirBuild(root: string): string
/** Makes the finished `build` the one `dist-air` names, and removes the builds before the last `keep` older ones */
export function publishAirBuild(root: string, build: string, options?: { keep?: number }): void
