import { z } from "zod"

/**
 * Schema for test repository setup options with clear defaults
 */
export const SetupRepoOptionsSchema = z.object({
  /**
   * Whether to populate the Tauri store with repository data
   * Default: true - Most tests need a populated store to simulate production
   */
  prepopulateStore: z.boolean().default(true),

  /**
   * Whether to create a recentProject entry for NO_REPO template
   * Default: false - NO_REPO typically used for welcome card (no recent projects)
   * Set to true for path validation tests that need a "saved but invalid" repository
   */
  createRecentProject: z.boolean().default(false),

  /**
   * What `suggest_branch_names` answers (`{ outcome?: BranchNameSuggestionOutcome, delayMs?: number }`, see
   * `ai-test-helpers.ts`), set before the first sync
   * Default: undefined - two names from the words of the commit messages after 100 ms
   */
  branchNameSuggestions: z.any().optional(),

  /**
   * How long ago (ms) the last sync of the repository ended, as the writer of the settings records it in
   * `repositoryState` (set through `/store` before the page opens): the window syncs a repository synced before by itself
   * Default: undefined - never synced, the Ready to Sync card waits for Sync
   */
  syncedAgoMs: z.number().optional(),

  /**
   * How many branches that last sync had (`lastBranchCount` of `repositoryState`), with `syncedAgoMs`: the table loads in
   * their room until the first sync's rows come
   * Default: undefined - not recorded, the loading table is one row tall
   */
  syncedBranchCount: z.number().optional(),

  /**
   * Additional store values to inject into the test
   * Default: empty object - Only used for specific test scenarios
   */
  initialStoreValues: z.record(z.string(), z.any()).default({}),

  /**
   * What the session's list of the pull requests answers (a `PullRequestsFixture`, see `pull-request-helpers.ts`), set
   * before the first sync
   * Default: undefined - the repository is not on GitHub
   */
  pullRequests: z.any().optional(),

  /**
   * The app's GitHub sign-in (a `GitHubSignInFixture` of `setGitHubSignIn`: the OAuth app of the build, the token file,
   * the token of `gh`), set before the page opens: the session reads the sign-ins when the window opens it
   * Default: undefined - a build without the OAuth app, signed in with the token of the host
   */
  gitHubSignIn: z.any().optional(),

  /**
   * What GitHub answers of the review requests (a `ReviewRequestsFixture`, see `review-request-helpers.ts`), set before the first sync
   * Default: undefined - the repository is not on GitHub
   */
  reviewRequests: z.any().optional(),

  /**
   * What the fake Patronus answers the session's lists and polls (`PatronusRobotsFixture` of `patronus-helpers.ts`),
   * set before the first sync
   * Default: undefined - the repository is not on Space
   */
  patronus: z.any().optional(),

  /**
   * The TeamCity checks and problems the fake Patronus answers for robots (see `TestRepositoryBuilder.setPatronusDetails`),
   * set before the first sync
   * Default: undefined - no robot has checks or problems
   */
  patronusDetails: z.any().optional(),

  /**
   * The merge profiles the fake Patronus offers for a start (see `TestRepositoryBuilder.setPatronusProfiles`), set before
   * the first sync
   * Default: undefined - no merge profile, only the default checks
   */
  patronusProfiles: z.any().optional(),

  /**
   * What the fake Patronus answers the code-owner pre-check with, by branch (see `TestRepositoryBuilder.setReviewAnswers`),
   * set before the first sync
   * Default: undefined - the owners of the files a branch changes by the `OWNERSHIP` files of the target
   */
  reviewAnswers: z.any().optional(),

  /**
   * The repository is in the middle of a rebase (`TestRepositoryBuilder.setRebaseInProgress`) when the page opens: its
   * syncs are deferred
   * Default: false
   */
  rebaseInProgress: z.boolean().default(false),

  /**
   * What the first sync reports differently (a `SyncOverrides`, see `TestRepositoryBuilder.setSyncOverrides`), set
   * before the page opens
   * Default: undefined - the first sync reports what the server computes
   */
  syncOverrides: z.any().optional(),

  /**
   * What the fake YouTrack holds (a `YouTrackFixture`, see `youtrack-helpers.ts`), set before the page opens, so the
   * window starts with its sign-in
   * Default: undefined - no issue, signed out
   */
  youtrack: z.any().optional(),
})

/**
 * Input type for setup options (what users pass in)
 */
export type SetupRepoOptions = z.input<typeof SetupRepoOptionsSchema>

/**
 * Parsed type with defaults applied (what code uses internally)
 */
export type ParsedSetupRepoOptions = z.output<typeof SetupRepoOptionsSchema>

/**
 * Parse and validate setup options, applying defaults
 */
export function parseSetupOptions(options?: SetupRepoOptions): ParsedSetupRepoOptions {
  return SetupRepoOptionsSchema.parse(options ?? {})
}
