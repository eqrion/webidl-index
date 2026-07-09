import { useMemo, useState } from 'preact/hooks'
import { search } from '../search'
import type { SearchEntry } from '../types'

interface Row {
  name: string
  via?: string
}

interface Props {
  names: string[]
  /** Once loaded, search also matches member/field names and types, not
   * just the definition's own name. Falls back to a plain name filter
   * while it's still loading. */
  searchIndex: SearchEntry[] | null
  selected?: string
  onSelect: (name: string) => void
}

export function DefinitionList({ names, searchIndex, selected, onSelect }: Props) {
  const [filter, setFilter] = useState('')

  const rows: Row[] = useMemo(() => {
    const q = filter.trim()
    if (!q) return names.map((name) => ({ name }))
    if (searchIndex) {
      return search(searchIndex, q).map((r) => ({
        name: r.entry.name,
        via: r.via ? `${r.via.name || 'return'}: ${r.via.type}` : undefined,
      }))
    }
    const lower = q.toLowerCase()
    return names.filter((n) => n.toLowerCase().includes(lower)).map((name) => ({ name }))
  }, [names, searchIndex, filter])

  return (
    <div class="definition-list">
      <input
        type="text"
        placeholder={searchIndex ? `search names & members (${names.length})` : `filter (${names.length})`}
        value={filter}
        onInput={(e) => setFilter((e.target as HTMLInputElement).value)}
      />
      <ul>
        {rows.map((row) => (
          <li key={row.name} class={row.name === selected ? 'selected' : ''} onClick={() => onSelect(row.name)}>
            <span class="name">{row.name}</span>
            {row.via && <span class="via"> · {row.via}</span>}
          </li>
        ))}
      </ul>
    </div>
  )
}
