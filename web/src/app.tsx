import { useEffect, useState } from 'preact/hooks'
import './app.css'
import { getManifest } from './api'
import { BrowseView } from './components/BrowseView'
import { DiffPanel } from './components/DiffPanel'
import { MergeView } from './components/MergeView'
import { useHashView } from './router'
import type { Manifest } from './types'

export function App() {
  const [view, navigate] = useHashView()
  const [manifest, setManifest] = useState<Manifest | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getManifest()
      .then(setManifest)
      .catch((e: unknown) => setError(String(e)))
  }, [])

  return (
    <div class="app">
      <header>
        <h1>WebIDL Index</h1>
        <nav>
          <button
            type="button"
            class={view.mode === 'browse' ? 'active' : ''}
            onClick={() => navigate({ mode: 'browse', engine: 'webref', version: 'current' })}
          >
            Browse
          </button>
          <button
            type="button"
            class={view.mode === 'merge' ? 'active' : ''}
            onClick={() => navigate({ mode: 'merge', snapshots: [] })}
          >
            Intersect
          </button>
          <button
            type="button"
            class={view.mode === 'diff' ? 'active' : ''}
            onClick={() => navigate({ mode: 'diff', engineA: '', versionA: '', engineB: '', versionB: '' })}
          >
            Diff
          </button>
        </nav>
      </header>
      <main>
        {error && <div class="error">Failed to load manifest: {error}</div>}
        {!error && !manifest && <div class="hint">Loading…</div>}
        {manifest && view.mode === 'browse' && <BrowseView manifest={manifest} view={view} navigate={navigate} />}
        {manifest && view.mode === 'merge' && <MergeView manifest={manifest} view={view} navigate={navigate} />}
        {manifest && view.mode === 'diff' && <DiffPanel manifest={manifest} view={view} navigate={navigate} />}
      </main>
    </div>
  )
}
