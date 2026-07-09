import { baselineSet, type SnapshotRef } from '../merge'
import type { Manifest } from '../types'

interface Props {
  manifest: Manifest
  selected: SnapshotRef[]
  isBaseline: boolean
  onChange: (snapshots: SnapshotRef[]) => void
}

/** Multi-select of an arbitrary (engine, version) combination, one
 * `<select multiple>` per engine so long version lists (145+ for Blink)
 * scroll instead of sprawling. */
export function SnapshotPicker({ manifest, selected, isBaseline, onChange }: Props) {
  const engines = Object.keys(manifest.engines).sort()
  const selectedByEngine = new Map<string, Set<string>>()
  for (const s of selected) {
    if (!selectedByEngine.has(s.engine)) selectedByEngine.set(s.engine, new Set())
    selectedByEngine.get(s.engine)!.add(s.version)
  }

  function setEngineSelection(engine: string, versions: string[]) {
    onChange([...selected.filter((s) => s.engine !== engine), ...versions.map((version) => ({ engine, version }))])
  }

  return (
    <div class="snapshot-picker">
      {engines.map((engine) => (
        <label key={engine} class="snapshot-picker-engine">
          <span class="snapshot-picker-label">{engine}</span>
          <select
            multiple
            size={4}
            onChange={(e) => {
              const options = Array.from((e.target as HTMLSelectElement).selectedOptions)
              setEngineSelection(engine, options.map((o) => o.value))
            }}
          >
            {(manifest.engines[engine] ?? []).map((v) => (
              <option key={v.version} value={v.version} selected={selectedByEngine.get(engine)?.has(v.version)}>
                {v.version}
              </option>
            ))}
          </select>
        </label>
      ))}
      <button type="button" class={`baseline-reset ${isBaseline ? 'active' : ''}`} onClick={() => onChange(baselineSet(manifest))}>
        Reset to baseline
      </button>
    </div>
  )
}
