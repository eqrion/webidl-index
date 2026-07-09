import { useEffect, useState } from 'preact/hooks'
import { getDefinition, getSearchIndex, getSnapshot } from '../api'
import type { View } from '../router'
import type { Definition, Manifest, SearchEntry, Snapshot } from '../types'
import { DefinitionList } from './DefinitionList'
import { DefinitionView } from './DefinitionView'
import { ExploreTree } from './ExploreTree'
import { VersionPicker } from './VersionPicker'

interface Props {
  manifest: Manifest
  view: Extract<View, { mode: 'browse' }>
  navigate: (view: View) => void
}

type ListMode = 'all' | 'global'

export function BrowseView({ manifest, view, navigate }: Props) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [searchIndex, setSearchIndex] = useState<SearchEntry[] | null>(null)
  const [def, setDef] = useState<Definition | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [listMode, setListMode] = useState<ListMode>('all')

  useEffect(() => {
    setSnapshot(null)
    setSearchIndex(null)
    setErr(null)
    if (!view.engine || !view.version) return
    getSnapshot(view.engine, view.version)
      .then(setSnapshot)
      .catch((e: unknown) => setErr(String(e)))
    // Lazy: the search index is much larger than the snapshot itself, and
    // isn't needed until the user actually searches or opens "By Global".
    getSearchIndex(view.engine, view.version)
      .then(setSearchIndex)
      .catch((e: unknown) => setErr(String(e)))
  }, [view.engine, view.version])

  useEffect(() => {
    setDef(null)
    if (!view.engine || !view.version || !view.name) return
    getDefinition(view.engine, view.version, view.name)
      .then(setDef)
      .catch((e: unknown) => setErr(String(e)))
  }, [view.engine, view.version, view.name])

  const names = snapshot ? Object.keys(snapshot.entries).sort() : []

  return (
    <div class="browse">
      <div class="toolbar">
        <VersionPicker
          manifest={manifest}
          engine={view.engine}
          version={view.version}
          onChange={(engine, version) => navigate({ mode: 'browse', engine, version })}
        />
        {snapshot && (
          <span class="source">
            {snapshot.source.tag} · {snapshot.date.slice(0, 10)}
            {snapshot.parse_errors?.length ? ` · ${snapshot.parse_errors.length} parse warning(s)` : ''}
          </span>
        )}
      </div>
      {err && <div class="error">{err}</div>}
      <div class="panes">
        <div class="list-pane">
          <div class="list-mode-toggle">
            <button type="button" class={listMode === 'all' ? 'active' : ''} onClick={() => setListMode('all')}>
              All
            </button>
            <button type="button" class={listMode === 'global' ? 'active' : ''} onClick={() => setListMode('global')}>
              By Global
            </button>
          </div>
          {listMode === 'all' ? (
            <DefinitionList
              names={names}
              searchIndex={searchIndex}
              selected={view.name}
              onSelect={(name) => navigate({ ...view, name })}
            />
          ) : searchIndex ? (
            <ExploreTree index={searchIndex} selected={view.name} onSelect={(name) => navigate({ ...view, name })} />
          ) : (
            <div class="hint">Loading…</div>
          )}
        </div>
        <div class="detail">
          {def ? (
            <DefinitionView def={def} knownNames={new Set(names)} onNavigate={(name) => navigate({ ...view, name })} />
          ) : (
            <div class="hint">Select a definition</div>
          )}
        </div>
      </div>
    </div>
  )
}
