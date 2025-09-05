import type { z } from "zod"
import type { ReactiveStoreMetadata } from "./app-store"

/**
 * Creates a synchronous persistent reactive store that automatically saves to and loads from storage
 * @param key - Storage key for persisting the state
 * @param metadata - Reactive store metadata (schema, isArray, isMainOnly)
 * @returns Reactive state object
 */
export function createPersistentStore<T extends z.ZodTypeAny>(
  key: string,
  metadata: ReactiveStoreMetadata & { schema: T },
): z.infer<T> {
  // Register reactive store - handles all value logic and persistence internally
  return appStore.registerReactiveStore(key, metadata)
}
