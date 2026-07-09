import { definitionLines } from '../diff'
import { kindLabel } from '../format'
import type { Definition } from '../types'
import { CodeTokens } from './CodeTokens'

interface Props {
  def: Definition
  knownNames?: Set<string>
  onNavigate?: (name: string) => void
}

export function DefinitionView({ def, knownNames, onNavigate }: Props) {
  return (
    <div>
      <div class="def-kind">{kindLabel(def)}</div>
      <pre class="code">
        {definitionLines(def).map((line, i) => (
          <div key={i} class="code-line">
            {line.indent > 0 && <span class="code-indent" style={{ width: `${line.indent * 20}px` }} />}
            <CodeTokens tokens={line.tokens} knownNames={knownNames} onNavigate={onNavigate} />
          </div>
        ))}
      </pre>
    </div>
  )
}
