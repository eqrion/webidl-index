import { fieldLine, headerLine, memberLine, type TokenLine } from './format'
import { tokensToText, type Token } from './tokens'
import type { Definition } from './types'

export interface DefLine {
  tokens: Token[]
  indent: number
}

const CLOSE: DefLine = { tokens: [{ text: '};', kind: 'punct' }], indent: 0 }

/** Expands a declaration into its own extended-attributes line (if any),
 * followed by its body line, both at the given indent. */
function expand(line: TokenLine, indent: number): DefLine[] {
  const lines: DefLine[] = []
  if (line.attrs) lines.push({ tokens: line.attrs, indent })
  lines.push({ tokens: line.body, indent })
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
      return [...expand(headerLine(def), 0), ...def.members.flatMap((m) => expand(memberLine(m), 1)), CLOSE]
    case 'dictionary':
      return [...expand(headerLine(def), 0), ...def.fields.flatMap((f) => expand(fieldLine(f), 1)), CLOSE]
    case 'enum':
      return [
        ...expand(headerLine(def), 0),
        ...def.values.map((v) => ({ tokens: [{ text: `"${v}"`, kind: 'string' as const }, { text: ';', kind: 'punct' as const }], indent: 1 })),
        CLOSE,
      ]
    case 'typedef':
    case 'callback':
      return expand(headerLine(def), 0)
  }
}

export interface DiffLine {
  tokens: Token[]
  indent: number
  status: 'added' | 'removed' | 'unchanged'
}

export function diffLines(aLines: DefLine[], bLines: DefLine[]): DiffLine[] {
  const aByText = new Map(aLines.map((l) => [tokensToText(l.tokens), l]))
  const bByText = new Map(bLines.map((l) => [tokensToText(l.tokens), l]))
  const union = Array.from(new Set([...aByText.keys(), ...bByText.keys()])).sort()
  return union.map((text) => {
    const inA = aByText.has(text)
    const inB = bByText.has(text)
    const line = (inA ? aByText.get(text) : bByText.get(text))!
    return { tokens: line.tokens, indent: line.indent, status: inA && inB ? 'unchanged' : inA ? 'removed' : 'added' }
  })
}

export function diffDefinitions(a: Definition, b: Definition): DiffLine[] {
  return diffLines(definitionLines(a), definitionLines(b))
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
