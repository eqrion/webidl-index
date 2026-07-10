import type { DiffLine } from '../diff'
import { CodeTokens } from './CodeTokens'

function marker(status: DiffLine['status']): string {
  return status === 'added' ? '+' : status === 'removed' ? '-' : ' '
}

// This diff is computed as (A, B): `added` means only in B (B has it, extra
// relative to A), `removed` means only in A (A has it, missing from B).
// Both read naturally in terms of B alone, since B is "compared against" A.
function title(status: DiffLine['status'], labelB: string): string | undefined {
  return status === 'added' ? `Extra in ${labelB}` : status === 'removed' ? `Missing from ${labelB}` : undefined
}

interface Props {
  lines: DiffLine[]
  labelB: string
  knownNames?: Set<string>
  onNavigate?: (name: string) => void
}

export function DiffLinesView({ lines, labelB, knownNames, onNavigate }: Props) {
  return (
    <pre class="code diff">
      {lines.map((l, i) => (
        <div key={i} class={`diff-line diff-${l.status}`} title={title(l.status, labelB)}>
          <span class="diff-marker">{marker(l.status)}</span>
          {l.indent > 0 && <span class="code-indent" style={{ width: `${l.indent * 20}px` }} />}
          <CodeTokens tokens={l.tokens} knownNames={knownNames} onNavigate={onNavigate} />
        </div>
      ))}
    </pre>
  )
}
