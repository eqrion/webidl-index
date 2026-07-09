import type { Manifest } from '../types'

interface Props {
  manifest: Manifest
  engine: string
  version: string
  onChange: (engine: string, version: string) => void
}

export function VersionPicker({ manifest, engine, version, onChange }: Props) {
  const engines = Object.keys(manifest.engines).sort()
  const versions = manifest.engines[engine] ?? []

  return (
    <span class="picker">
      <select
        value={engine}
        onChange={(e) => {
          const nextEngine = (e.target as HTMLSelectElement).value
          const nextVersions = manifest.engines[nextEngine] ?? []
          onChange(nextEngine, nextVersions.at(-1)?.version ?? '')
        }}
      >
        <option value="" disabled>
          engine
        </option>
        {engines.map((e) => (
          <option key={e} value={e}>
            {e}
          </option>
        ))}
      </select>
      <select
        value={version}
        disabled={!engine}
        onChange={(e) => onChange(engine, (e.target as HTMLSelectElement).value)}
      >
        <option value="" disabled>
          version
        </option>
        {versions.map((v) => (
          <option key={v.version} value={v.version}>
            {v.version}
          </option>
        ))}
      </select>
    </span>
  )
}
