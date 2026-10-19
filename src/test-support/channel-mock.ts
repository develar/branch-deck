/** `@tauri-apps/api/core` for store tests: a `Channel` whose `onmessage` the mocked command calls with its events */
export class Channel<T = unknown> {
  onmessage: (event: T) => void = () => {}
}
