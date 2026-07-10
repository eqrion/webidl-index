import { fieldLine, headerLine, memberLine, type TokenLine } from './format'
import { tokensToText, type Token } from './tokens'
import type { Argument, Definition, Field, Member } from './types'
import { isStandardExtendedAttribute } from './webidlAttributes'

// Renders the normalized model into tokenized WebIDL-ish lines. This is a
// display convenience only -- it doesn't need to be a byte-perfect WebIDL
// grammar, just readable and close enough that a WebIDL reader recognizes it
// immediately.

/** Every rendered line belongs to one of these buckets, so a diff can be
 * filtered/summarized finer than whole-entry added/removed/changed (e.g.
 * "extra members" vs "extra enum values"). `structural` (the closing brace)
 * is deliberately excluded from `CATEGORIES` -- it always duplicates the
 * `declaration` line's added/removed status for a wholly-added/removed
 * entry, and counting it too would double-count those entries. */
export type LineCategory = 'declaration' | 'member' | 'field' | 'value' | 'structural'

export const CATEGORIES: readonly LineCategory[] = ['declaration', 'member', 'field', 'value']

export const CATEGORY_LABELS: Record<LineCategory, string> = {
  declaration: 'declarations',
  member: 'members',
  field: 'fields',
  value: 'enum values',
  structural: 'structural',
}

export interface DefLine {
  tokens: Token[]
  indent: number
  category: LineCategory
}

const CLOSE: DefLine = { tokens: [{ text: '};', kind: 'punct' }], indent: 0, category: 'structural' }

/** Extended attributes render as their own `[Foo, Bar]` line ahead of the
 * declaration they annotate, so `null` here means "no attrs line needed". */
function expand(line: TokenLine, indent: number, category: LineCategory): DefLine[] {
  const lines: DefLine[] = []
  if (line.attrs) lines.push({ tokens: line.attrs, indent, category })
  lines.push({ tokens: line.body, indent, category })
  return lines
}

// Every definition kind is flattened to a list of lines: its header, one
// line per member/field/value, and a closing line for symmetry. Two
// definitions are compared purely by set-diffing those lines' rendered text:
// a line present on both sides is unchanged, present only on one side is
// added/removed. This means an edit to one member shows as a paired
// remove+add rather than an in-place "changed" line, but it needs no
// per-member identity beyond its own rendered text, which member overloads
// and dictionary fields already have.
export function definitionLines(def: Definition): DefLine[] {
  switch (def.kind) {
    case 'interface':
    case 'callback_interface':
    case 'namespace':
      return [
        ...expand(headerLine(def), 0, 'declaration'),
        ...def.members.flatMap((m) => expand(memberLine(m), 1, 'member')),
        CLOSE,
      ]
    case 'dictionary':
      return [...expand(headerLine(def), 0, 'declaration'), ...def.fields.flatMap((f) => expand(fieldLine(f), 1, 'field')), CLOSE]
    case 'enum':
      return [
        ...expand(headerLine(def), 0, 'declaration'),
        ...def.values.map((v) => ({
          tokens: [{ text: `"${v}"`, kind: 'string' as const }, { text: ';', kind: 'punct' as const }],
          indent: 1,
          category: 'value' as const,
        })),
        CLOSE,
      ]
    case 'typedef':
    case 'callback':
      return expand(headerLine(def), 0, 'declaration')
  }
}

export interface DiffLine {
  tokens: Token[]
  indent: number
  status: 'added' | 'removed' | 'unchanged'
  category: LineCategory
}

/** Aligns the two line lists by their longest common subsequence (matching
 * lines by rendered text), so the result reads in declaration order --
 * header, then members/fields/values, then the closing brace -- instead of
 * an arbitrary alphabetical shuffle of unrelated lines. */
export function diffLines(aLines: DefLine[], bLines: DefLine[]): DiffLine[] {
  const aText = aLines.map((l) => tokensToText(l.tokens))
  const bText = bLines.map((l) => tokensToText(l.tokens))
  const n = aLines.length
  const m = bLines.length

  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0))
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = aText[i] === bText[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1])
    }
  }

  const result: DiffLine[] = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (aText[i] === bText[j]) {
      result.push({ ...aLines[i], status: 'unchanged' })
      i++
      j++
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      result.push({ ...aLines[i], status: 'removed' })
      i++
    } else {
      result.push({ ...bLines[j], status: 'added' })
      j++
    }
  }
  while (i < n) result.push({ ...aLines[i++], status: 'removed' })
  while (j < m) result.push({ ...bLines[j++], status: 'added' })
  return result
}

function filterAttrs(attrs: string[]): string[] {
  return attrs.filter(isStandardExtendedAttribute)
}

function stripArg(a: Argument): Argument {
  return { ...a, extended_attributes: filterAttrs(a.extended_attributes) }
}

function stripMember(m: Member): Member {
  return { ...m, extended_attributes: filterAttrs(m.extended_attributes), arguments: m.arguments.map(stripArg) }
}

function stripField(f: Field): Field {
  return { ...f, extended_attributes: filterAttrs(f.extended_attributes) }
}

/** Drops engine-internal extended attributes (Gecko's `Pref`/`ChromeOnly`,
 * Blink's `RuntimeEnabled`/`MeasureAs`, etc.) before diffing, so comparing
 * an engine against webref -- or against another engine -- isn't dominated
 * by annotations that aren't part of the actual API surface. Only used for
 * diffing; the plain Browse/Intersect views still show every attribute. */
function stripEngineAttributes(def: Definition): Definition {
  const extended_attributes = filterAttrs(def.extended_attributes)
  switch (def.kind) {
    case 'interface':
    case 'callback_interface':
    case 'namespace':
      return { ...def, extended_attributes, members: def.members.map(stripMember) }
    case 'dictionary':
      return { ...def, extended_attributes, fields: def.fields.map(stripField) }
    case 'enum':
    case 'typedef':
      return { ...def, extended_attributes }
    case 'callback':
      return { ...def, extended_attributes, arguments: def.arguments.map(stripArg) }
  }
}

function diffableLines(def: Definition | null): DefLine[] {
  return def ? definitionLines(stripEngineAttributes(def)) : []
}

/** Diffs two definitions, tolerating either side being absent (the entry
 * exists in only one snapshot). The absent side contributes no lines, so
 * every line from the present side comes out wholly added/removed -- the
 * same code path as a partial change, just total. */
export function diffDefinitions(a: Definition | null, b: Definition | null): DiffLine[] {
  return diffLines(diffableLines(a), diffableLines(b))
}

export interface EntryDiff {
  name: string
  status: 'added' | 'removed' | 'changed'
}

/** Compares two snapshots' `entries` maps (name -> content hash). */
export function diffEntries(a: Record<string, string>, b: Record<string, string>): EntryDiff[] {
  const names = new Set([...Object.keys(a), ...Object.keys(b)])
  const result: EntryDiff[] = []
  for (const name of names) {
    const ha = a[name]
    const hb = b[name]
    if (ha === undefined) result.push({ name, status: 'added' })
    else if (hb === undefined) result.push({ name, status: 'removed' })
    else if (ha !== hb) result.push({ name, status: 'changed' })
  }
  return result.sort((x, y) => x.name.localeCompare(y.name))
}

export interface CategoryCounts {
  added: number
  removed: number
}

export type CategoryStats = Record<LineCategory, CategoryCounts>

function emptyCategoryStats(): CategoryStats {
  return {
    declaration: { added: 0, removed: 0 },
    member: { added: 0, removed: 0 },
    field: { added: 0, removed: 0 },
    value: { added: 0, removed: 0 },
    structural: { added: 0, removed: 0 },
  }
}

/** Line-level category breakdown of one entry's diff (independent of its
 * whole-entry added/removed/changed status), so the compare view can filter
 * and summarize finer than "this whole interface is extra". Counting doesn't
 * need declaration order, so this is a plain set difference rather than
 * `diffLines`' O(n*m) alignment -- it runs once per differing entry across
 * an entire compare, which can be thousands of definitions. */
export function categorizeDefinitions(a: Definition | null, b: Definition | null): CategoryStats {
  const aLines = diffableLines(a)
  const bLines = diffableLines(b)
  const aByText = new Map(aLines.map((l) => [tokensToText(l.tokens), l]))
  const bByText = new Map(bLines.map((l) => [tokensToText(l.tokens), l]))
  const stats = emptyCategoryStats()
  for (const [text, line] of aByText) {
    if (!bByText.has(text)) stats[line.category].removed++
  }
  for (const [text, line] of bByText) {
    if (!aByText.has(text)) stats[line.category].added++
  }
  return stats
}

export function mergeCategoryStats(list: CategoryStats[]): CategoryStats {
  const total = emptyCategoryStats()
  for (const s of list) {
    for (const cat of CATEGORIES) {
      total[cat].added += s[cat].added
      total[cat].removed += s[cat].removed
    }
  }
  return total
}
