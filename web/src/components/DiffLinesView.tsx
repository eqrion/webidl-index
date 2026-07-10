import type { DiffLine } from '../diff'
import { CodeTokens } from './CodeTokens'

function marker(status: DiffLine['status']): string {
  return status === 'added' ? '+' : status === 'removed' ? '-' : ' '
}

// See DiffPanel's statusLabel: this diff is computed as (A, B), so `added`
// means only in B and `removed` means only in A.
function title(status: DiffLine['status']): string | undefined {
  return status === 'added' ? 'Only in B' : status === 'removed' ? 'Only in A' : undefined
}

interface Props {
  lines: DiffLine[]
  knownNames?: Set<string>
  onNavigate?: (name: string) => void
}

export function DiffLinesView({ lines, knownNames, onNavigate }: Props) {
  return (
    <pre class="code diff">
      {lines.map((l, i) => (
        <div key={i} class={`diff-line diff-${l.status}`} title={title(l.status)}>
          <span class="diff-marker">{marker(l.status)}</span>
          {l.indent > 0 && <span class="code-indent" style={{ width: `${l.indent * 20}px` }} />}
          <CodeTokens tokens={l.tokens} knownNames={knownNames} onNavigate={onNavigate} />
        </div>
      ))}
    </pre>
  )
}
