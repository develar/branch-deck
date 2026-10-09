import { readdirSync, readFileSync } from "node:fs"
import { createRequire } from "node:module"
import { join } from "node:path"

/**
 * Icon identity for HTML snapshots.
 *
 * Nuxt UI renders icons as inline SVG through `@iconify/vue`, and the icon name is not present in the DOM.
 * This index maps a hash of each lucide icon body (as the browser serializes it) to an icon name,
 * so the snapshot formatter can print which icon is shown (see `iconName` in `tauri-mocks.ts`).
 *
 * Alias names share a body with their target icon (e.g. `help-circle` and `circle-help`).
 * Names used in the app sources win, so snapshots show the name the code asks for.
 */

// FNV-1a, must match `hashIconBody` in tauri-mocks.ts
export function hashIconBody(text: string): string {
  let hash = 0x811C9DC5
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16)
}

// The browser serializes `<path .../>` from innerHTML as `<path ...></path>`
function toSerializedForm(body: string): string {
  return body.replace(/<([a-zA-Z][\w:-]*)((?:\s+[^\s=>/]+="[^"]*")*)\s*\/>/g, "<$1$2></$1>")
}

function collectSourceIconNames(root: string): Set<string> {
  const names = new Set<string>()
  const visit = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) {
        visit(path)
      }
      else if (/\.(vue|ts)$/.test(entry.name)) {
        for (const match of readFileSync(path, "utf8").matchAll(/i-lucide-([a-z0-9-]+)/g)) {
          names.add(match[1]!)
        }
      }
    }
  }
  visit(join(root, "app"))
  visit(join(root, "layers"))
  return names
}

let cached: Record<string, string> | undefined

export function buildIconNameIndex(): Record<string, string> {
  if (cached) {
    return cached
  }

  const root = process.cwd()
  const require = createRequire(join(root, "package.json"))
  const collection = require("@iconify-json/lucide/icons.json") as {
    icons: Record<string, { body: string }>
    aliases?: Record<string, { parent: string }>
  }

  const preferred = collectSourceIconNames(root)
  const index: Record<string, string> = {}
  const add = (name: string, body: string, override: boolean) => {
    const key = hashIconBody(toSerializedForm(body))
    if (override || !(key in index)) {
      index[key] = name
    }
  }

  for (const [name, icon] of Object.entries(collection.icons)) {
    add(name, icon.body, false)
  }
  for (const name of preferred) {
    let target = name
    for (let depth = 0; depth < 5 && collection.aliases?.[target]; depth++) {
      target = collection.aliases[target]!.parent
    }
    const icon = collection.icons[target]
    if (icon) {
      add(name, icon.body, true)
    }
  }

  cached = index
  return index
}
