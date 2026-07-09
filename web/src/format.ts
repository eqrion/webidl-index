import { tokenizeType, type Token, type TokenKind } from './tokens'
import type { Argument, Definition, Field, Member } from './types'

// Renders the normalized model into tokenized WebIDL-ish lines. This is a
// display convenience only -- it doesn't need to be a byte-perfect WebIDL
// grammar, just readable and close enough that a WebIDL reader recognizes it
// immediately.

function t(text: string, kind: TokenKind): Token {
  return { text, kind }
}

export interface TokenLine {
  attrs: Token[] | null
  body: Token[]
}

/** Extended attributes render as their own `[Foo, Bar]` line ahead of the
 * declaration they annotate, so `null` here means "no attrs line needed". */
function attrTokens(list: string[]): Token[] | null {
  if (!list.length) return null
  const toks: Token[] = [t('[', 'punct')]
  list.forEach((a, i) => {
    if (i > 0) toks.push(t(', ', 'punct'))
    toks.push(t(a, 'attr'))
  })
  toks.push(t(']', 'punct'))
  return toks
}

function argTokens(list: Argument[]): Token[] {
  const toks: Token[] = []
  list.forEach((a, i) => {
    if (i > 0) toks.push(t(', ', 'punct'))
    if (a.optional) toks.push(t('optional ', 'keyword'))
    toks.push(...tokenizeType(a.type_))
    if (a.variadic) toks.push(t('...', 'punct'))
    toks.push(t(' ', 'punct'), t(a.name, 'name'))
    if (a.default !== undefined) toks.push(t(' = ', 'punct'), t(a.default, 'value'))
  })
  return toks
}

export function memberLine(m: Member): TokenLine {
  const toks: Token[] = []
  if (m.modifiers.length) toks.push(t(`${m.modifiers.join(' ')} `, 'keyword'))
  switch (m.kind) {
    case 'const':
      toks.push(t('const ', 'keyword'), ...tokenizeType(m.type_!), t(' ', 'punct'), t(m.name, 'name'), t(' = ', 'punct'), t(m.value!, 'value'), t(';', 'punct'))
      break
    case 'attribute':
      toks.push(t('attribute ', 'keyword'), ...tokenizeType(m.type_!), t(' ', 'punct'), t(m.name, 'name'), t(';', 'punct'))
      break
    case 'constructor':
      toks.push(t('constructor(', 'keyword'), ...argTokens(m.arguments), t(');', 'punct'))
      break
    case 'operation':
      toks.push(...tokenizeType(m.type_!), t(' ', 'punct'), t(m.name, 'name'), t('(', 'punct'), ...argTokens(m.arguments), t(');', 'punct'))
      break
    case 'iterable':
      toks.push(...tokenizeType(m.type_!), t(';', 'punct'))
      break
    case 'async_iterable':
      toks.push(...tokenizeType(m.type_!))
      if (m.arguments.length) toks.push(t('(', 'punct'), ...argTokens(m.arguments), t(')', 'punct'))
      toks.push(t(';', 'punct'))
      break
    case 'maplike':
    case 'setlike':
      toks.push(...tokenizeType(m.type_!), t(';', 'punct'))
      break
    case 'stringifier':
      toks.push(t('stringifier;', 'keyword'))
      break
  }
  return { attrs: attrTokens(m.extended_attributes), body: toks }
}

export function fieldLine(f: Field): TokenLine {
  const toks: Token[] = []
  if (f.required) toks.push(t('required ', 'keyword'))
  toks.push(...tokenizeType(f.type_), t(' ', 'punct'), t(f.name, 'name'))
  if (f.default !== undefined) toks.push(t(' = ', 'punct'), t(f.default, 'value'))
  toks.push(t(';', 'punct'))
  return { attrs: attrTokens(f.extended_attributes), body: toks }
}

function inheritsTokens(name: string | undefined): Token[] {
  return name ? [t(' : ', 'punct'), ...tokenizeType(name)] : []
}

export function headerLine(def: Definition): TokenLine {
  const toks: Token[] = []
  switch (def.kind) {
    case 'interface':
      toks.push(t('interface ', 'keyword'), t(def.name, 'name'), ...inheritsTokens(def.inherits), t(' {', 'punct'))
      break
    case 'callback_interface':
      toks.push(t('callback interface ', 'keyword'), t(def.name, 'name'), t(' {', 'punct'))
      break
    case 'namespace':
      toks.push(t('namespace ', 'keyword'), t(def.name, 'name'), t(' {', 'punct'))
      break
    case 'dictionary':
      toks.push(t('dictionary ', 'keyword'), t(def.name, 'name'), ...inheritsTokens(def.inherits), t(' {', 'punct'))
      break
    case 'enum':
      toks.push(t('enum ', 'keyword'), t(def.name, 'name'), t(' {', 'punct'))
      break
    case 'typedef':
      toks.push(t('typedef ', 'keyword'), ...tokenizeType(def.aliased_type), t(' ', 'punct'), t(def.name, 'name'), t(';', 'punct'))
      break
    case 'callback':
      toks.push(
        t('callback ', 'keyword'),
        t(def.name, 'name'),
        t(' = ', 'punct'),
        ...tokenizeType(def.return_type),
        t('(', 'punct'),
        ...argTokens(def.arguments),
        t(');', 'punct'),
      )
      break
  }
  return { attrs: attrTokens(def.extended_attributes), body: toks }
}

export function kindLabel(def: Definition): string {
  switch (def.kind) {
    case 'callback_interface':
      return 'callback interface'
    case 'callback':
      return 'callback'
    default:
      return def.kind
  }
}
