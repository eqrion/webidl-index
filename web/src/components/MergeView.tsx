import { useEffect, useMemo, useState } from 'preact/hooks'
import { getDefinition, getObject, getSearchIndex, getSnapshot } from '../api'
import { baselineSet, mergeDefinition, mergeEntryNames, mergeSearchIndex, sharedHash, type SnapshotRef } from '../merge'
import type { View } from '../router'
import type { Definition, Manifest, SearchEntry, Snapshot } from '../types'
import { DefinitionList } from './DefinitionList'
import { DefinitionView } from './DefinitionView'
import { ExploreTree } from './ExploreTree'
import { SnapshotPicker } from './SnapshotPicker'

interface Props {
  manifest: Manifest
  view: Extract<View, { mode: 'merge' }>
  navigate: (view: View) => void
}

type ListMode = 'all' | 'global'

function sameSet(a: SnapshotRef[], b: SnapshotRef[]): boolean {
  if (a.length !== b.length) return false
  const bKeys = new Set(b.map((s) => `${s.engine}:${s.version}`))
  return a.every((s) => bKeys.has(`${s.engine}:${s.version}`))
}

/** Renders the common subset of an arbitrary set of snapshots. `view.snapshots`
 * empty means "baseline" (last few majors of every engine, resolved below) --
 * this is also the app's default landing view. Structurally this mirrors
 * `BrowseView`, but its three inputs (entry list, selected definition, search
 * index) are each merged client-side (see `merge.ts`) instead of read
 * straight off one snapshot. */
export function MergeView({ manifest, view, navigate }: Props) {
  const isBaseline = view.snapshots.length === 0
  const refs = isBaseline ? baselineSet(manifest) : view.snapshots
  const refsKey = refs.map((r) => `${r.engine}:${r.version}`).join(',')

  const [snapshots, setSnapshots] = useState<Snapshot[] | null>(null)
  const [searchIndex, setSearchIndex] = useState<SearchEntry[] | null>(null)
  const [def, setDef] = useState<Definition | null>(null)
  const [noCommonDef, setNoCommonDef] = useState(false)
  const [err, setErr] = useState<string | null>(null)
  const [listMode, setListMode] = useState<ListMode>('all')

  useEffect(() => {
    setSnapshots(null)
    setSearchIndex(null)
    setErr(null)
    if (refs.length === 0) return
    Promise.all(refs.map((r) => getSnapshot(r.engine, r.version)))
      .then(setSnapshots)
      .catch((e: unknown) => setErr(String(e)))
    Promise.all(refs.map((r) => getSearchIndex(r.engine, r.version)))
      .then((indices) => setSearchIndex(mergeSearchIndex(indices)))
      .catch((e: unknown) => setErr(String(e)))
    // refsKey captures the same identity as `refs`; depending on it instead
    // of `refs` itself avoids re-fetching every render.
  }, [refsKey])

  const names = useMemo(() => (snapshots ? mergeEntryNames(snapshots.map((s) => s.entries)) : []), [snapshots])

  useEffect(() => {
    setDef(null)
    setNoCommonDef(false)
    const name = view.name
    if (!snapshots || !name) return
    const entries = snapshots.map((s) => s.entries)
    const shared = sharedHash(entries, name)
    const load = shared ? getObject(shared).then((d) => [d]) : Promise.all(refs.map((r) => getDefinition(r.engine, r.version, name)))
    load
      .then((defs) => {
        const merged = mergeDefinition(defs)
        if (merged) setDef(merged)
        else setNoCommonDef(true)
      })
      .catch((e: unknown) => setErr(String(e)))
    // refsKey (not `refs`) is the real dependency; see note above.
  }, [snapshots, view.name, refsKey])

  function handleSnapshotsChange(next: SnapshotRef[]) {
    const baseline = baselineSet(manifest)
    navigate({ ...view, snapshots: sameSet(next, baseline) ? [] : next })
  }

  const engineCount = new Set(refs.map((r) => r.engine)).size

  return (
    <div class="browse merge-view">
      <div class="toolbar">
        <SnapshotPicker manifest={manifest} selected={refs} isBaseline={isBaseline} onChange={handleSnapshotsChange} />
        <span class="source">
          {isBaseline ? 'Intersect · ' : 'Custom · '}
          common subset of {refs.length} snapshot{refs.length === 1 ? '' : 's'} across {engineCount} engine{engineCount === 1 ? '' : 's'}
        </span>
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
          ) : noCommonDef ? (
            <div class="hint">No common definition across the selected snapshots (absent from one, or a different kind in another).</div>
          ) : (
            <div class="hint">Select a definition</div>
          )}
        </div>
      </div>
    </div>
  )
}
