import { getObject } from './api'
import { categorizeDefinitions, type CategoryStats, type EntryDiff } from './diff'

async function mapPool<T, R>(items: T[], limit: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array(items.length)
  let next = 0
  async function worker() {
    while (next < items.length) {
      const i = next++
      results[i] = await fn(items[i])
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker))
  return results
}

/** Fetches both sides' definitions for every entry that differs between the
 * two snapshots and categorizes the line-level differences, so the compare
 * view can filter/summarize by category (members, fields, enum values...)
 * instead of only by whole-entry status. Capped concurrency since this can
 * mean thousands of small object fetches for a large diff. */
export async function computeEntryCategoryStats(
  entryDiff: EntryDiff[],
  entriesA: Record<string, string>,
  entriesB: Record<string, string>,
): Promise<Map<string, CategoryStats>> {
  const pairs = await mapPool(entryDiff, 24, async (e) => {
    const hashA = entriesA[e.name]
    const hashB = entriesB[e.name]
    const [defA, defB] = await Promise.all([hashA ? getObject(hashA) : null, hashB ? getObject(hashB) : null])
    return [e.name, categorizeDefinitions(defA, defB)] as const
  })
  return new Map(pairs)
}
