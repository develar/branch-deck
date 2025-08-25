import { z } from "zod"
import { createPersistentStore } from "~/utils/persistent-store"

// Schema for ProjectMetadata with normalization
const ProjectMetadataSchema = z.object({
  path: z.string(),
  cachedBranchPrefix: z.string().optional().transform(val => val === "" ? undefined : val),
  lastSyncTime: z.number().optional(),
  lastBranchCount: z.number().optional(),
  issueNavigationConfig: z.any().optional(), // TODO: create proper schema
})

// Schema for repository settings - just the array directly
const RepositorySettingsSchema = z.array(ProjectMetadataSchema).default([])

// Create the persistent store synchronously
export const repositorySettings = createPersistentStore(
  "recentProjects", // Keep the same key for backward compatibility
  { schema: RepositorySettingsSchema, isArray: true, isMainOnly: false },
)

export type ProjectMetadata = z.infer<typeof ProjectMetadataSchema>