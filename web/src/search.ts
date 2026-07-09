import type { SearchChild, SearchEntry } from './types'

export interface SearchResult {
  entry: SearchEntry
  score: number
  /** Set when the match came from a member/field rather than the name itself. */
  via?: SearchChild
}

/**
 * Ranks name matches above member/field matches, and exact/prefix matches
 * above plain substrings, so searching "window" doesn't bury the Window
 * interface itself under every member merely mentioning it in a type.
 */
export function search(entries: SearchEntry[], query: string, limit = 60): SearchResult[] {
  const q = query.trim().toLowerCase()
  if (!q) return []

  const results: SearchResult[] = []
  for (const entry of entries) {
    const name = entry.name.toLowerCase()
    if (name === q) {
      results.push({ entry, score: 100 })
      continue
    }
    if (name.startsWith(q)) {
      results.push({ entry, score: 80 })
      continue
    }
    if (name.includes(q)) {
      results.push({ entry, score: 60 })
      continue
    }
    const nameMatch = entry.children.find((c) => c.name.toLowerCase().includes(q))
    if (nameMatch) {
      results.push({ entry, score: 40, via: nameMatch })
      continue
    }
    const typeMatch = entry.children.find((c) => c.type.toLowerCase().includes(q))
    if (typeMatch) {
      results.push({ entry, score: 20, via: typeMatch })
    }
  }

  results.sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name))
  return results.slice(0, limit)
}
