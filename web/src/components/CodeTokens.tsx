import type { Token } from '../tokens'

interface Props {
  tokens: Token[]
  /** Definition names known to exist in the current snapshot(s). Only type
   * tokens matching a known name become clickable cross-links. */
  knownNames?: Set<string>
  onNavigate?: (name: string) => void
}

export function CodeTokens({ tokens, knownNames, onNavigate }: Props) {
  return (
    <>
      {tokens.map((tok, i) =>
        tok.link && onNavigate && knownNames?.has(tok.link) ? (
          <span key={i} class={`tok tok-${tok.kind} tok-link`} onClick={() => onNavigate(tok.link!)}>
            {tok.text}
          </span>
        ) : (
          <span key={i} class={`tok tok-${tok.kind}`}>
            {tok.text}
          </span>
        ),
      )}
    </>
  )
}
