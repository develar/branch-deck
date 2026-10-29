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

/**
 * The forms of a path from the whole to the shortest, each cut at a separator, never in a segment: `~/scratch/demo`,
 * `…/demo`, then the name alone, `demo`, for a room too narrow for `…/demo`. A cut drops more than the one character
 * of "…": `…/scratch/demo` would only trade `~` for it
 */
export function pathForms(path: string): string[] {
  const forms = [path]
  const trimmed = path.replace(/[/\\]+$/, "")
  for (let index = 2; index < trimmed.length; index++) {
    // the last separator of a run: `/srv//demo` gives `…/demo`
    if (isSeparator(trimmed[index]) && !isSeparator(trimmed[index + 1])) {
      forms.push(`…${trimmed.slice(index)}`)
    }
  }
  const nameStart = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\")) + 1
  if (nameStart > 0 && nameStart < trimmed.length) {
    forms.push(trimmed.slice(nameStart))
  }
  return forms
}

function isSeparator(char: string | undefined): boolean {
  return char === "/" || char === "\\"
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
