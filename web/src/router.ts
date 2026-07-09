import { useCallback, useEffect, useState } from 'preact/hooks'
import type { SnapshotRef } from './merge'

export type View =
  | { mode: 'browse'; engine: string; version: string; name?: string }
  | { mode: 'diff'; engineA: string; versionA: string; engineB: string; versionB: string; name?: string }
  | { mode: 'merge'; snapshots: SnapshotRef[]; name?: string }

/** `snapshots: []` is the baseline sentinel -- MergeView resolves it to
 * `baselineSet(manifest)` once the manifest has loaded. This is also the
 * app's default landing view. */
const EMPTY_MERGE: View = { mode: 'merge', snapshots: [] }

/** Renders a merge set as one hash segment, e.g. `blink:139,gecko:140`.
 * Each engine/version is percent-encoded individually so a literal `:` or
 * `,` inside one (never happens in practice, but keeps the round-trip
 * exact) can't be confused with the separators. */
function encodeSnapshotSet(snapshots: SnapshotRef[]): string {
  return snapshots.map((s) => `${encodeURIComponent(s.engine)}:${encodeURIComponent(s.version)}`).join(',')
}

function decodeSnapshotSet(segment: string): SnapshotRef[] {
  if (!segment || segment === '-') return []
  return segment.split(',').flatMap((pair) => {
    const [engine, version] = pair.split(':')
    return engine && version ? [{ engine: decodeURIComponent(engine), version: decodeURIComponent(version) }] : []
  })
}

export function parseHash(hash: string): View {
  const parts = hash.replace(/^#\/?/, '').split('/').filter(Boolean)

  if (parts[0] === 'diff') {
    const [, engineA = '', versionA = '', engineB = '', versionB = '', name] = parts.map(decodeURIComponent)
    return { mode: 'diff', engineA, versionA, engineB, versionB, name }
  }
  if (parts[0] === 'merge') {
    const snapshots = decodeSnapshotSet(parts[1] ?? '')
    return { mode: 'merge', snapshots, name: parts[2] ? decodeURIComponent(parts[2]) : undefined }
  }
  if (parts[0] === 'browse') {
    const [, engine = '', version = '', name] = parts.map(decodeURIComponent)
    return { mode: 'browse', engine, version, name }
  }
  return EMPTY_MERGE
}

export function formatHash(view: View): string {
  if (view.mode === 'diff') {
    const parts = ['diff', view.engineA, view.versionA, view.engineB, view.versionB]
    if (view.name) parts.push(view.name)
    return `#/${parts.map(encodeURIComponent).join('/')}`
  }
  if (view.mode === 'merge') {
    const setSegment = encodeSnapshotSet(view.snapshots)
    const parts = ['merge']
    if (setSegment || view.name) parts.push(setSegment || '-')
    if (view.name) parts.push(encodeURIComponent(view.name))
    return `#/${parts.join('/')}`
  }
  const parts = ['browse', view.engine, view.version]
  if (view.name) parts.push(view.name)
  return `#/${parts.map(encodeURIComponent).join('/')}`
}

/** Hash-based routing so every view is a shareable, bookmarkable URL. */
export function useHashView(): [View, (view: View) => void] {
  const [view, setView] = useState<View>(() => parseHash(location.hash))

  useEffect(() => {
    const onChange = () => setView(parseHash(location.hash))
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])

  const navigate = useCallback((next: View) => {
    location.hash = formatHash(next)
  }, [])

  return [view, navigate]
}
