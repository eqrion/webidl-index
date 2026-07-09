export type TokenKind = 'keyword' | 'type' | 'name' | 'punct' | 'string' | 'value' | 'attr'

export interface Token {
  text: string
  kind: TokenKind
  /** Bare identifier this token might name. Set only on identifier chunks
   * within a type expression, so the renderer can cross-link it to a
   * matching definition if one exists in the current snapshot(s). */
  link?: string
}

/** Splits a type expression (e.g. `sequence<HTMLElement>?`) into identifier
 * and punctuation tokens, so each identifier can be colored and potentially
 * cross-linked independently of the surrounding generics/punctuation. */
export function tokenizeType(type: string): Token[] {
  return type
    .split(/([A-Za-z_][A-Za-z0-9_]*)/)
    .filter((s) => s.length > 0)
    .map((s) => (/^[A-Za-z_]/.test(s) ? { text: s, kind: 'type', link: s } : { text: s, kind: 'punct' }))
}

export function tokensToText(tokens: Token[]): string {
  return tokens.map((tok) => tok.text).join('')
}
