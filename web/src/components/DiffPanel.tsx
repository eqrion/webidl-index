import { useEffect, useMemo, useState } from 'preact/hooks'
import { getObject, getSnapshot } from '../api'
import { computeEntryCategoryStats } from '../comparestats'
import { CATEGORIES, CATEGORY_LABELS, diffDefinitions, diffEntries, mergeCategoryStats, type CategoryStats, type EntryDiff, type LineCategory } from '../diff'
import type { View } from '../router'
import type { Definition, Manifest, Snapshot } from '../types'
import { DiffLinesView } from './DiffLinesView'
import { VersionPicker } from './VersionPicker'

interface Props {
  manifest: Manifest
  view: Extract<View, { mode: 'diff' }>
  navigate: (view: View) => void
}

type Direction = 'added' | 'removed'
type FilterKey = `${LineCategory}:${Direction}`

const ALL_FILTERS = new Set<FilterKey>(CATEGORIES.flatMap((c) => [`${c}:added`, `${c}:removed`] as FilterKey[]))

function marker(status: EntryDiff['status']): string {
  return status === 'added' ? '+' : status === 'removed' ? '-' : '~'
}

// diffEntries/diffDefinitions are computed as (A, B): `added` means present
// only in B (extra, relative to A), `removed` means present only in A
// (missing from B). Both are phrased in terms of B alone -- B is "compared
// against" A -- so the reader never has to resolve an abstract "A"/"B".
function statusLabel(status: EntryDiff['status'], labelB: string): string {
  return status === 'added' ? `Extra in ${labelB}` : status === 'removed' ? `Missing from ${labelB}` : 'Changed'
}

function sideLabel(engine: string, version: string): string {
  return `${engine} ${version}`
}

function latestVersion(manifest: Manifest, engine: string): string {
  return manifest.engines[engine]?.at(-1)?.version ?? ''
}

export function DiffPanel({ manifest, view, navigate }: Props) {
  const [snapA, setSnapA] = useState<Snapshot | null>(null)
  const [snapB, setSnapB] = useState<Snapshot | null>(null)
  const [defA, setDefA] = useState<Definition | null>(null)
  const [defB, setDefB] = useState<Definition | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [stats, setStats] = useState<Map<string, CategoryStats> | null>(null)
  const [activeFilters, setActiveFilters] = useState<Set<FilterKey>>(ALL_FILTERS)
  const [search, setSearch] = useState('')

  const ready = Boolean(view.engineA && view.versionA && view.engineB && view.versionB)

  useEffect(() => {
    setSnapA(null)
    setSnapB(null)
    setErr(null)
    if (!ready) return
    Promise.all([getSnapshot(view.engineA, view.versionA), getSnapshot(view.engineB, view.versionB)])
      .then(([a, b]) => {
        setSnapA(a)
        setSnapB(b)
      })
      .catch((e: unknown) => setErr(String(e)))
  }, [ready, view.engineA, view.versionA, view.engineB, view.versionB])

  useEffect(() => {
    setDefA(null)
    setDefB(null)
    if (!snapA || !snapB || !view.name) return
    const hashA = snapA.entries[view.name]
    const hashB = snapB.entries[view.name]
    Promise.all([hashA ? getObject(hashA) : null, hashB ? getObject(hashB) : null])
      .then(([a, b]) => {
        setDefA(a)
        setDefB(b)
      })
      .catch((e: unknown) => setErr(String(e)))
  }, [snapA, snapB, view.name])

  const entryDiff = useMemo<EntryDiff[]>(() => (snapA && snapB ? diffEntries(snapA.entries, snapB.entries) : []), [snapA, snapB])

  useEffect(() => {
    setStats(null)
    if (!snapA || !snapB || entryDiff.length === 0) return
    let cancelled = false
    computeEntryCategoryStats(entryDiff, snapA.entries, snapB.entries).then((result) => {
      if (!cancelled) setStats(result)
    })
    return () => {
      cancelled = true
    }
  }, [snapA, snapB, entryDiff])

  const knownNames = new Set(entryDiff.map((e) => e.name))
  const onNavigate = (name: string) => navigate({ ...view, name })
  const labelB = sideLabel(view.engineB, view.versionB)

  const compareEngines = Object.keys(manifest.engines)
    .filter((e) => e !== 'webref')
    .sort()
  const webrefVersion = latestVersion(manifest, 'webref')
  // webref (the spec) is the base (A); the engine is B, so "extra in
  // {engine}"/"missing from {engine}" reads as "how does this engine differ
  // from the spec" -- the natural framing for this shortcut.
  const quickCompareValue =
    view.engineA === 'webref' && view.versionA === webrefVersion && view.versionB === latestVersion(manifest, view.engineB)
      ? view.engineB
      : ''

  const aggregate = stats ? mergeCategoryStats([...stats.values()]) : null

  // Plain click isolates to just the clicked cell/row/column -- one click
  // gets you "only extra members" -- shift-click adds/removes it from the
  // current selection instead, for combining a few together.
  function toggleKeys(keys: FilterKey[]) {
    setActiveFilters((prev) => {
      const allActive = keys.every((k) => prev.has(k))
      const next = new Set(prev)
      keys.forEach((k) => (allActive ? next.delete(k) : next.add(k)))
      return next
    })
  }

  function clickFilter(keys: FilterKey[], e: MouseEvent) {
    if (e.shiftKey) toggleKeys(keys)
    else setActiveFilters(new Set(keys))
  }

  function entryMatches(name: string): boolean {
    if (!stats) return true
    const s = stats.get(name)
    if (!s) return false
    return CATEGORIES.some(
      (cat) => (activeFilters.has(`${cat}:added`) && s[cat].added > 0) || (activeFilters.has(`${cat}:removed`) && s[cat].removed > 0),
    )
  }

  const query = search.trim().toLowerCase()
  const filteredEntries = entryDiff.filter((e) => entryMatches(e.name) && e.name.toLowerCase().includes(query))
  const addedEntries = entryDiff.filter((e) => e.status === 'added').length
  const removedEntries = entryDiff.filter((e) => e.status === 'removed').length
  const changedEntries = entryDiff.filter((e) => e.status === 'changed').length

  return (
    <div class="diff-panel">
      <div class="toolbar">
        {webrefVersion && (
          <>
            <select
              class="quick-compare"
              value={quickCompareValue}
              onChange={(e) => {
                const engine = (e.target as HTMLSelectElement).value
                if (!engine) return
                navigate({
                  ...view,
                  engineA: 'webref',
                  versionA: webrefVersion,
                  engineB: engine,
                  versionB: latestVersion(manifest, engine),
                })
              }}
            >
              <option value="">Quick compare…</option>
              {compareEngines.map((engine) => (
                <option key={engine} value={engine}>
                  {engine} (latest) vs webref
                </option>
              ))}
            </select>
            <span class="toolbar-divider" />
          </>
        )}
        <span class="diff-side-label" title="Base: markers show changes relative to this side">
          A
        </span>
        <VersionPicker
          manifest={manifest}
          engine={view.engineA}
          version={view.versionA}
          onChange={(engineA, versionA) => navigate({ ...view, engineA, versionA })}
        />
        <span class="vs" title="A is the base; B is compared against it">
          →
        </span>
        <span class="diff-side-label" title="Compared against A">
          B
        </span>
        <VersionPicker
          manifest={manifest}
          engine={view.engineB}
          version={view.versionB}
          onChange={(engineB, versionB) => navigate({ ...view, engineB, versionB })}
        />
      </div>
      {err && <div class="error">{err}</div>}
      <div class="panes">
        <div class="definition-list">
          <input
            type="text"
            placeholder={`filter by name (${entryDiff.length})`}
            value={search}
            onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
          />
          {entryDiff.length > 0 && (
            <div class="summary">
              <div class="summary-counts">
                {addedEntries} new · {removedEntries} removed · {changedEntries} changed
                <span
                  class="summary-hint"
                  title="Engine-internal extended attributes (Gecko's Pref/ChromeOnly, Blink's RuntimeEnabled/MeasureAs, etc.) are excluded from this diff -- only WebIDL/HTML-spec attributes are compared."
                >
                  {' '}
                  · internal attributes hidden ⓘ
                </span>
              </div>
              <table class="stats-table">
                <thead>
                  <tr>
                    <th>
                      <button
                        type="button"
                        class="stats-reset"
                        disabled={activeFilters.size === ALL_FILTERS.size}
                        title="Show all categories and directions"
                        onClick={() => setActiveFilters(ALL_FILTERS)}
                      >
                        Reset
                      </button>
                    </th>
                    <th>
                      <button
                        type="button"
                        title="Click to show only extras; shift-click to add/remove this column"
                        onClick={(e) => clickFilter(CATEGORIES.map((c) => `${c}:added` as FilterKey), e)}
                      >
                        extra in {labelB}
                      </button>
                    </th>
                    <th>
                      <button
                        type="button"
                        title="Click to show only what's missing; shift-click to add/remove this column"
                        onClick={(e) => clickFilter(CATEGORIES.map((c) => `${c}:removed` as FilterKey), e)}
                      >
                        missing from {labelB}
                      </button>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {CATEGORIES.map((cat) => (
                    <tr key={cat}>
                      <td class="stats-row-label">
                        <button
                          type="button"
                          title={`Click to show only ${CATEGORY_LABELS[cat]}; shift-click to add/remove this row`}
                          onClick={(e) => clickFilter([`${cat}:added`, `${cat}:removed`], e)}
                        >
                          {CATEGORY_LABELS[cat]}
                        </button>
                      </td>
                      {(['added', 'removed'] as const).map((dir) => {
                        const key: FilterKey = `${cat}:${dir}`
                        return (
                          <td key={dir}>
                            <button
                              type="button"
                              class={`stat-toggle ${activeFilters.has(key) ? 'active' : ''}`}
                              disabled={!stats}
                              title={
                                !stats
                                  ? 'Computing member-level differences…'
                                  : 'Click to show only this; shift-click to add/remove it'
                              }
                              onClick={(e) => clickFilter([key], e)}
                            >
                              {aggregate ? aggregate[cat][dir] : '…'}
                            </button>
                          </td>
                        )
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <ul>
            {filteredEntries.map((e) => (
              <li
                key={e.name}
                class={`diff-entry diff-${e.status} ${e.name === view.name ? 'selected' : ''}`}
                onClick={() => navigate({ ...view, name: e.name })}
                title={statusLabel(e.status, labelB)}
              >
                <span class="diff-marker">{marker(e.status)}</span>
                {e.name}
              </li>
            ))}
          </ul>
        </div>
        <div class="detail">
          {view.name && (defA || defB) ? (
            <DiffLinesView lines={diffDefinitions(defA, defB)} labelB={labelB} knownNames={knownNames} onNavigate={onNavigate} />
          ) : view.name ? (
            <div class="hint">Loading…</div>
          ) : (
            <div class="hint">Select a definition</div>
          )}
        </div>
      </div>
    </div>
  )
}
