import { useMemo, useState } from 'preact/hooks'
import { findExposed, findGlobals, resolveTypeNames } from '../explore'
import type { SearchChild, SearchEntry } from '../types'

interface Props {
  index: SearchEntry[]
  selected?: string
  onSelect: (name: string) => void
}

export function ExploreTree({ index, selected, onSelect }: Props) {
  const byName = useMemo(() => new Map(index.map((e) => [e.name, e])), [index])
  const globals = useMemo(() => findGlobals(index), [index])
  const [chosen, setChosen] = useState('')

  const defaultGlobal = globals.find((g) => g.name === 'Window')?.name ?? globals[0]?.name ?? ''
  const activeGlobal = chosen || defaultGlobal
  const rootEntry = byName.get(activeGlobal)
  // Everything exposed on a global is often hundreds of entries (~870 for
  // Window in a typical Gecko snapshot) -- only the root itself defaults to
  // expanded, or the initial render would eagerly mount every member row of
  // every one of them.
  const exposed = useMemo(
    () => (activeGlobal ? findExposed(index, activeGlobal).filter((e) => e.name !== activeGlobal) : []),
    [index, activeGlobal],
  )

  if (globals.length === 0) {
    return <div class="hint">No [Global] interfaces found in this snapshot.</div>
  }

  return (
    <div class="explore-tree">
      <select class="global-picker" value={activeGlobal} onChange={(e) => setChosen((e.target as HTMLSelectElement).value)}>
        {globals.map((g) => (
          <option key={g.name} value={g.name}>
            {g.name}
          </option>
        ))}
      </select>
      <ul class="tree-root">
        {rootEntry && (
          <EntryNode entry={rootEntry} byName={byName} selected={selected} onSelect={onSelect} ancestors={EMPTY} defaultOpen />
        )}
        {exposed.length > 0 && <li class="tree-group-label">also exposed on {activeGlobal}</li>}
        {exposed.map((e) => (
          <EntryNode key={e.name} entry={e} byName={byName} selected={selected} onSelect={onSelect} ancestors={EMPTY} />
        ))}
      </ul>
    </div>
  )
}

const EMPTY: ReadonlySet<string> = new Set()

interface EntryNodeProps {
  entry: SearchEntry
  byName: Map<string, SearchEntry>
  selected?: string
  onSelect: (name: string) => void
  ancestors: ReadonlySet<string>
  defaultOpen?: boolean
}

function EntryNode({ entry, byName, selected, onSelect, ancestors, defaultOpen = false }: EntryNodeProps) {
  const [open, setOpen] = useState(defaultOpen)
  const nextAncestors = useMemo(() => new Set([...ancestors, entry.name]), [ancestors, entry.name])
  const hasChildren = entry.children.length > 0

  return (
    <li class="tree-node">
      <div class="tree-row">
        <button type="button" class="tree-toggle" disabled={!hasChildren} onClick={() => setOpen((o) => !o)}>
          {hasChildren ? (open ? '▾' : '▸') : '·'}
        </button>
        <span class={`tree-label ${entry.name === selected ? 'selected' : ''}`} onClick={() => onSelect(entry.name)}>
          {entry.name}
        </span>
        <span class="tree-kind">{entry.kind}</span>
      </div>
      {open && hasChildren && (
        <ul>
          {entry.children.map((child, i) => (
            <ChildNode key={`${child.name}:${i}`} child={child} byName={byName} selected={selected} onSelect={onSelect} ancestors={nextAncestors} />
          ))}
        </ul>
      )}
    </li>
  )
}

interface ChildNodeProps {
  child: SearchChild
  byName: Map<string, SearchEntry>
  selected?: string
  onSelect: (name: string) => void
  ancestors: ReadonlySet<string>
}

function ChildNode({ child, byName, selected, onSelect, ancestors }: ChildNodeProps) {
  const targets = useMemo(() => resolveTypeNames(child.type, byName), [child.type, byName])
  const [open, setOpen] = useState(false)
  const expandable = targets.some((t) => !ancestors.has(t.name))

  return (
    <li class="tree-node tree-child">
      <div class="tree-row">
        <button type="button" class="tree-toggle" disabled={!expandable} onClick={() => setOpen((o) => !o)}>
          {expandable ? (open ? '▾' : '▸') : '·'}
        </button>
        <span class="tree-member-name">{child.name || '(return)'}</span>
        <span class="tree-member-type">: {child.type}</span>
      </div>
      {open && (
        <ul>
          {targets
            .filter((t) => !ancestors.has(t.name))
            .map((t) => (
              <EntryNode key={t.name} entry={t} byName={byName} selected={selected} onSelect={onSelect} ancestors={ancestors} />
            ))}
          {targets.some((t) => ancestors.has(t.name)) && <li class="tree-cycle-hint">↩ already shown above</li>}
        </ul>
      )}
    </li>
  )
}
