import type { Definition, Manifest, SearchEntry, Snapshot } from './types'

// data/ is served alongside the built app (see vite.config.ts and
// .github/workflows/deploy.yml), so every path here is relative to the
// site root, prefixed with Vite's base for subpath deployments.
function dataUrl(path: string): string {
  return `${import.meta.env.BASE_URL}data/${path}`
}

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url)
  if (!res.ok) {
    throw new Error(`${url}: ${res.status} ${res.statusText}`)
  }
  return res.json() as Promise<T>
}

let manifestPromise: Promise<Manifest> | null = null

export function getManifest(): Promise<Manifest> {
  manifestPromise ??= fetchJson<Manifest>(dataUrl('manifest.json'))
  return manifestPromise
}

const snapshotCache = new Map<string, Promise<Snapshot>>()

export function getSnapshot(engine: string, version: string): Promise<Snapshot> {
  const key = `${engine}/${version}`
  let promise = snapshotCache.get(key)
  if (!promise) {
    promise = fetchJson<Snapshot>(dataUrl(`snapshots/${engine}/${version}.json`))
    snapshotCache.set(key, promise)
  }
  return promise
}

const objectCache = new Map<string, Promise<Definition>>()

export function getObject(hash: string): Promise<Definition> {
  let promise = objectCache.get(hash)
  if (!promise) {
    promise = fetchJson<Definition>(dataUrl(`objects/${hash.slice(0, 2)}/${hash}.json`))
    objectCache.set(hash, promise)
  }
  return promise
}

const searchIndexCache = new Map<string, Promise<SearchEntry[]>>()

/** Fetched lazily (only once search or the explore tree is actually used),
 * since it's much larger than the snapshot itself. */
export function getSearchIndex(engine: string, version: string): Promise<SearchEntry[]> {
  const key = `${engine}/${version}`
  let promise = searchIndexCache.get(key)
  if (!promise) {
    promise = fetchJson<SearchEntry[]>(dataUrl(`snapshots/${engine}/${version}.index.json`))
    searchIndexCache.set(key, promise)
  }
  return promise
}

let webrefNamesPromise: Promise<Set<string>> | null = null

/** The webref snapshot's entry names double as an oracle for "does
 * webidlpedia (which is generated from webref) have a page for this name",
 * without any extra network call beyond the snapshot itself. */
export function getWebrefNames(): Promise<Set<string>> {
  webrefNamesPromise ??= getManifest().then((manifest) => {
    const version = manifest.engines.webref?.[0]?.version
    if (!version) return new Set<string>()
    return getSnapshot('webref', version).then((snapshot) => new Set(Object.keys(snapshot.entries)))
  })
  return webrefNamesPromise
}

export function getDefinition(engine: string, version: string, name: string): Promise<Definition> {
  return getSnapshot(engine, version).then((snapshot) => {
    const hash = snapshot.entries[name]
    if (!hash) {
      throw new Error(`${name} not found in ${engine} ${version}`)
    }
    return getObject(hash)
  })
}
