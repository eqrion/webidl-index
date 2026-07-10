import { useEffect, useState } from 'preact/hooks'
import { getWebrefNames } from '../api'
import type { Definition } from '../types'

interface Props {
  name: string
  kind: Definition['kind']
}

const INTERFACE_LIKE: Definition['kind'][] = ['interface', 'callback_interface', 'namespace']

/** Links out to human-readable docs for a definition. webidlpedia only has
 * pages for names covered by webref, so we only link there when the webref
 * snapshot confirms the name exists; MDN has no such index, so it always
 * gets a link, falling back to search when we can't be sure a direct page
 * exists. */
export function ExternalLinks({ name, kind }: Props) {
  const [webrefNames, setWebrefNames] = useState<Set<string> | null>(null)

  useEffect(() => {
    getWebrefNames().then(setWebrefNames)
  }, [])

  const inWebref = webrefNames?.has(name) ?? false
  const encoded = encodeURIComponent(name)

  const mdnUrl =
    INTERFACE_LIKE.includes(kind) && inWebref
      ? `https://developer.mozilla.org/en-US/docs/Web/API/${encoded}`
      : `https://developer.mozilla.org/en-US/search?q=${encoded}`

  return (
    <div class="def-links">
      {inWebref && (
        <a
          class="def-link"
          href={`https://dontcallmedom.github.io/webidlpedia/names/${encoded}.html`}
          target="_blank"
          rel="noopener noreferrer"
        >
          webidlpedia ↗
        </a>
      )}
      <a class="def-link" href={mdnUrl} target="_blank" rel="noopener noreferrer">
        MDN ↗
      </a>
    </div>
  )
}
