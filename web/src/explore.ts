import type { SearchEntry } from './types'

const GLOBAL_RE = /^Global(?:=(.+))?$/
const EXPOSED_RE = /^Exposed=(.+)$/

function parseValueList(raw: string): string[] {
  const trimmed = raw.trim()
  if (trimmed.startsWith('(') && trimmed.endsWith(')')) {
    return trimmed
      .slice(1, -1)
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
  }
  return [trimmed]
}

export interface GlobalInfo {
  /** The name other definitions reference in their own `Exposed=` attribute. */
  name: string
  entry: SearchEntry
}

/**
 * Interfaces/namespaces declared `[Global]` or `[Global=Name(s)]`, one entry
 * per unique name (two different global scopes can share a name, e.g. both
 * audio and paint worklets declare `Global=(Worklet, ...)` -- only the first
 * is kept, since the name itself is all `findExposed` needs).
 */
export function findGlobals(index: SearchEntry[]): GlobalInfo[] {
  const seen = new Set<string>()
  const out: GlobalInfo[] = []
  for (const entry of index) {
    for (const attr of entry.extended_attributes) {
      const m = GLOBAL_RE.exec(attr)
      if (!m) continue
      const names = m[1] ? parseValueList(m[1]) : [entry.name]
      for (const name of names) {
        if (seen.has(name)) continue
        seen.add(name)
        out.push({ name, entry })
      }
    }
  }
  out.sort((a, b) => a.name.localeCompare(b.name))
  return out
}

/** Other definitions `[Exposed=...]` on the given global name (or `*`). */
export function findExposed(index: SearchEntry[], globalName: string): SearchEntry[] {
  return index.filter((entry) => {
    for (const attr of entry.extended_attributes) {
      const m = EXPOSED_RE.exec(attr)
      if (!m) continue
      const value = m[1].trim()
      if (value === '*') return true
      if (parseValueList(value).includes(globalName)) return true
    }
    return false
  })
}

const TOKEN_RE = /[A-Za-z_][A-Za-z0-9_]*/g

/**
 * Extracts identifiers from a rendered type string and resolves any that
 * name another definition in the same snapshot, e.g. `Promise<Response>`
 * resolves to the Response interface. Wrapper keywords like `sequence` or
 * `Promise` never match a real definition, so they're filtered out for free
 * by the "exists in this snapshot" check.
 */
export function resolveTypeNames(type: string, byName: Map<string, SearchEntry>): SearchEntry[] {
  const seen = new Set<string>()
  const out: SearchEntry[] = []
  for (const [token] of type.matchAll(TOKEN_RE)) {
    const entry = byName.get(token)
    if (entry && !seen.has(entry.name)) {
      seen.add(entry.name)
      out.push(entry)
    }
  }
  return out
}
