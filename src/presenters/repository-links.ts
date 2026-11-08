import type { RepositoryLink } from "@/lib/bindings"
import { repositoryName } from "@/lib/repository-config"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"

export interface ProjectItem {
  project: ProjectMetadata
  link: RepositoryLink | null
  isRecent: boolean
}

export interface ProjectSection {
  label: string | null
  items: ProjectItem[]
}

/** Recent projects and linked repositories; a linked project appears only once */
export function projectSections(current: string | null, links: readonly RepositoryLink[], recent: ProjectMetadata[]): ProjectSection[] {
  if (links.length === 0) {
    return [{ label: null, items: recent.map(project => ({ project, link: null, isRecent: true })) }]
  }

  const recentByPath = new Map<string, ProjectMetadata>()
  for (const project of recent) {
    const key = normalizedPath(project.path)
    if (!recentByPath.has(key)) {
      recentByPath.set(key, project)
    }
  }

  const currentPath = current === null ? null : normalizedPath(current)
  const linkedByPath = new Map<string, RepositoryLink>()
  for (const link of links) {
    const key = normalizedPath(link.path)
    if (key !== currentPath && !linkedByPath.has(key)) {
      linkedByPath.set(key, link)
    }
  }

  if (linkedByPath.size === 0) {
    return [{ label: null, items: recent.map(project => ({ project, link: null, isRecent: true })) }]
  }

  const linkedPaths = new Set(linkedByPath.keys())
  const linkedItems = [...linkedByPath].map(([key, link]) => ({
    project: recentByPath.get(key) ?? { path: link.path },
    link,
    isRecent: recentByPath.has(key),
  }))
  const recentItems = recent.filter(project => !linkedPaths.has(normalizedPath(project.path))).map(project => ({ project, link: null, isRecent: true }))
  const label = linkedItems.some(item => item.link.role === "primary") ? "Primary and companions" : current === null ? "Companions" : `Companions of ${repositoryName(current)}`

  return [...(recentItems.length === 0 ? [] : [{ label: "Recent", items: recentItems }]), { label, items: linkedItems }]
}

function normalizedPath(path: string): string {
  const normalized = path.replace(/\\/g, "/")
  if (/^[A-Za-z]:\/+$/u.test(normalized)) {
    return `${normalized.slice(0, 2)}/`
  }
  return normalized.replace(/\/+$/u, "") || (normalized.startsWith("/") ? "/" : normalized)
}
