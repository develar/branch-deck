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
   * Model download state for AI features
   * Default: undefined - Only set when testing AI-specific functionality
   */
  modelState: z.enum(["not_downloaded", "downloaded", "downloading"]).optional(),

  /**
   * Additional store values to inject into the test
   * Default: empty object - Only used for specific test scenarios
   */
  initialStoreValues: z.record(z.string(), z.any()).default({}),
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