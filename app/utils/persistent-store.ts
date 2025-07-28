import { watchDebounced } from "@vueuse/core"
import type { z } from "zod"

/**
 * Creates a persistent reactive store that automatically saves to and loads from storage
 * @param key - Storage key for persisting the state
 * @param schema - Zod schema for validation and defaults
 * @param storeName - Display name for error messages
 * @returns Promise that resolves to reactive state object after initial load
 */
export async function createAsyncPersistentStore<T extends z.ZodRawShape>(
  key: string,
  schema: z.ZodObject<T>,
  storeName: string,
): Promise<z.infer<typeof schema>> {
  type StateType = z.infer<typeof schema>

  // Load saved state or use empty object
  let initialData = null
  try {
    initialData = await appStore.get(key) ?? {}
    if (process.env.NUXT_PUBLIC_TEST_MODE) {
      console.log(`[PersistentStore] ${storeName} loading, saved:`, initialData)
    }
  }
  catch (error) {
    console.error(`Failed to load ${storeName} settings:`, error)
    // Use empty object on error, schema will apply defaults
    initialData = {}
  }

  // Parse through schema (applies defaults for missing fields)
  const parsedState = schema.parse(initialData)

  // Create reactive state with parsed data
  const state = reactive<StateType>(parsedState) as StateType

  if (process.env.NUXT_PUBLIC_TEST_MODE) {
    console.log(`[PersistentStore] ${storeName} initial state:`, parsedState)
  }

  // Parse empty object to get defaults for comparison
  const defaults = schema.parse({})
  watchDebounced(
    state,
    async (newState) => {
      try {
        const raw = toRaw(newState)

        // Debug logging in test mode
        if (process.env.NUXT_PUBLIC_TEST_MODE) {
          console.log(`[PersistentStore] ${storeName} changed:`, raw)
        }
        // Check if all values are defaults
        const isAllDefaults = Object.entries(raw).every(
          ([k, v]) => v === defaults[k as keyof StateType],
        )

        if (isAllDefaults) {
          // Remove from storage if all defaults
          await appStore.set(key, null)
        }
        else {
          // Only save non-default values
          const toSave: Record<string, unknown> = {}

          Object.entries(raw).forEach(([k, v]) => {
            if (v !== defaults[k as keyof StateType]) {
              toSave[k] = v
            }
          })

          await appStore.set(key, toSave)
        }
      }
      catch (error) {
        // Use notifyError if available in this context
        console.error(`Failed to save ${storeName} settings:`, error)
      }
    },
    {
      debounce: 500, // Wait 500ms after last change
      deep: true, // Watch nested properties
      immediate: false, // Don't trigger on initial value
    },
  )

  return state
}

/**
 * Creates a persistent reactive store that automatically saves to and loads from storage
 * @param key - Storage key for persisting the state
 * @param schema - Zod schema for validation and defaults
 * @param storeName - Display name for error messages
 * @returns Reactive state object
 */
export function createPersistentStore<T extends z.ZodRawShape>(
  key: string,
  schema: z.ZodObject<T>,
  storeName: string,
): z.infer<typeof schema> {
  type StateType = z.infer<typeof schema>

  // Get defaults from schema
  const defaults = schema.parse({})

  // Create reactive state with defaults
  const state = reactive<StateType>(defaults)

  // Load saved state asynchronously
  void (async () => {
    try {
      const saved = await appStore.get(key)
      if (process.env.NUXT_PUBLIC_TEST_MODE) {
        console.log(`[PersistentStore] ${storeName} loading, saved:`, saved)
      }
      if (saved) {
        // Parse and validate saved data
        const parsed = schema.parse(saved)
        // Merge into reactive state
        Object.assign(state, parsed)
        if (process.env.NUXT_PUBLIC_TEST_MODE) {
          console.log(`[PersistentStore] ${storeName} loaded state:`, parsed)
        }
      }
    }
    catch (error) {
      console.error(`Failed to load ${storeName} settings:`, error)
      // Use defaults on error
    }

    // Set up watching AFTER initial load completes
    // Use flush: 'post' to ensure we don't trigger on initial state
    watchDebounced(
      state,
      async (newState) => {
        try {
          const raw = toRaw(newState)

          // Debug logging in test mode
          if (process.env.NUXT_PUBLIC_TEST_MODE) {
            console.log(`[PersistentStore] ${storeName} changed:`, raw)
          }

          // Check if all values are defaults
          const isAllDefaults = Object.entries(raw).every(
            ([k, v]) => v === defaults[k as keyof StateType],
          )

          if (isAllDefaults) {
            // Remove from storage if all defaults
            await appStore.set(key, null)
          }
          else {
            // Only save non-default values
            const toSave: Record<string, unknown> = {}

            Object.entries(raw).forEach(([k, v]) => {
              if (v !== defaults[k as keyof StateType]) {
                toSave[k] = v
              }
            })

            await appStore.set(key, toSave)
          }
        }
        catch (error) {
          // Use notifyError if available in this context
          console.error(`Failed to save ${storeName} settings:`, error)
        }
      },
      {
        debounce: 500, // Wait 500ms after last change
        deep: true, // Watch nested properties
        flush: "post", // Ensure we don't trigger on initial reactive state setup
      },
    )
  })()

  return state as StateType
}