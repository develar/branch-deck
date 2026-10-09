/**
 * Formats a git hash to show only the first 8 characters
 * This is the standard short hash length used throughout the application
 */
export function formatShortHash(hash: string): string {
  return hash.substring(0, 8)
}
