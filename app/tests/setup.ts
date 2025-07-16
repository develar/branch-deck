import { vi } from "vitest"

// Mock Tauri APIs
Object.defineProperty(window, "__TAURI_INTERNALS__", {
  value: {
    transformCallback: vi.fn(() => vi.fn()),
    invoke: vi.fn(),
  },
  writable: true,
})

// Mock Tauri event system
Object.defineProperty(window, "__TAURI__", {
  value: {
    event: {
      listen: vi.fn(() => Promise.resolve(vi.fn())),
      emit: vi.fn(() => Promise.resolve()),
    },
    core: {
      invoke: vi.fn(),
    },
  },
  writable: true,
})

// Mock Toast composable since it's used in error handling
vi.mock("#imports", () => ({
  useToast: vi.fn(() => ({
    add: vi.fn(),
    remove: vi.fn(),
    update: vi.fn(),
  })),
}))