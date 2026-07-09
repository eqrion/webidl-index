import { useEffect, useState } from 'preact/hooks'
import { getObject, getSnapshot } from '../api'
import { diffDefinitions, diffEntries, type EntryDiff } from '../diff'
import type { View } from '../router'
import type { Definition, Manifest, Snapshot } from '../types'
import { DefinitionView } from './DefinitionView'
import { DiffLinesView } from './DiffLinesView'
import { VersionPicker } from './VersionPicker'

interface Props {
  manifest: Manifest
  view: Extract<View, { mode: 'diff' }>
  navigate: (view: View) => void
}

function marker(status: EntryDiff['status']): string {
  return status === 'added' ? '+' : status === 'removed' ? '-' : '~'
}

export function DiffPanel({ manifest, view, navigate }: Props) {
  const [snapA, setSnapA] = useState<Snapshot | null>(null)
  const [snapB, setSnapB] = useState<Snapshot | null>(null)
  const [defA, setDefA] = useState<Definition | null>(null)
  const [defB, setDefB] = useState<Definition | null>(null)
  const [err, setErr] = useState<string | null>(null)

  const ready = Boolean(view.engineA && view.versionA && view.engineB && view.versionB)

  useEffect(() => {
    setSnapA(null)
    setSnapB(null)
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

  const entryDiff: EntryDiff[] = snapA && snapB ? diffEntries(snapA.entries, snapB.entries) : []
  const knownNames = new Set(entryDiff.map((e) => e.name))
  const onNavigate = (name: string) => navigate({ ...view, name })

  return (
    <div class="diff-panel">
      <div class="toolbar">
        <VersionPicker
          manifest={manifest}
          engine={view.engineA}
          version={view.versionA}
          onChange={(engineA, versionA) => navigate({ ...view, engineA, versionA })}
        />
        <span class="vs">vs</span>
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
          {entryDiff.length > 0 && (
            <div class="summary">
              {entryDiff.filter((e) => e.status === 'added').length} added ·{' '}
              {entryDiff.filter((e) => e.status === 'removed').length} removed ·{' '}
              {entryDiff.filter((e) => e.status === 'changed').length} changed
            </div>
          )}
          <ul>
            {entryDiff.map((e) => (
              <li
                key={e.name}
                class={`diff-entry diff-${e.status} ${e.name === view.name ? 'selected' : ''}`}
                onClick={() => navigate({ ...view, name: e.name })}
              >
                <span class="diff-marker">{marker(e.status)}</span>
                {e.name}
              </li>
            ))}
          </ul>
        </div>
        <div class="detail">
          {view.name && defA && defB && (
            <DiffLinesView lines={diffDefinitions(defA, defB)} knownNames={knownNames} onNavigate={onNavigate} />
          )}
          {view.name && defA && !defB && <DefinitionView def={defA} knownNames={knownNames} onNavigate={onNavigate} />}
          {view.name && !defA && defB && <DefinitionView def={defB} knownNames={knownNames} onNavigate={onNavigate} />}
          {!view.name && <div class="hint">Select a definition</div>}
        </div>
      </div>
    </div>
  )
}
