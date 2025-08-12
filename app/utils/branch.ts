// Shared branch utilities
// Nuxt 4 auto-imports utils from app/utils, so getSimpleBranchName can be used directly in SFCs.

export function getSimpleBranchName(fullName: string): string {
  const lastSlash = fullName.lastIndexOf("/")
  return lastSlash === -1 ? fullName : fullName.slice(lastSlash + 1)
}
