import { logError } from "@/lib/errors"

/** Replaces the home directory with `~`: the whole path, or a path under it, never a sibling (`/Users/ann2`) */
export function tildePath(path: string, homeDir: string): string {
  const home = homeDir.replace(/[/\\]+$/, "")
  if (home.length === 0 || !path.startsWith(home)) {
    return path
  }
  const rest = path.slice(home.length)
  return rest.length === 0 || rest.startsWith("/") || rest.startsWith("\\") ? `~${rest}` : path
}

/** The user's home directory, `""` when Tauri can't tell it */
export async function resolveHomeDir(): Promise<string> {
  try {
    const { homeDir } = await import("@tauri-apps/api/path")
    return await homeDir()
  } catch (error: unknown) {
    logError("Failed to resolve home directory", error)
    return ""
  }
}
