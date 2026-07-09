export type MemberKind =
  | 'const'
  | 'attribute'
  | 'constructor'
  | 'operation'
  | 'iterable'
  | 'async_iterable'
  | 'maplike'
  | 'setlike'
  | 'stringifier'

export interface Argument {
  name: string
  type_: string
  optional: boolean
  variadic: boolean
  default?: string
  extended_attributes: string[]
}

export interface Member {
  kind: MemberKind
  name: string
  type_?: string
  arguments: Argument[]
  modifiers: string[]
  value?: string
  extended_attributes: string[]
}

export interface Field {
  name: string
  type_: string
  required: boolean
  default?: string
  extended_attributes: string[]
}

interface DefinitionBase {
  name: string
  extended_attributes: string[]
}

export type Definition =
  | (DefinitionBase & { kind: 'interface'; inherits?: string; members: Member[] })
  | (DefinitionBase & { kind: 'callback_interface'; members: Member[] })
  | (DefinitionBase & { kind: 'namespace'; members: Member[] })
  | (DefinitionBase & { kind: 'dictionary'; inherits?: string; fields: Field[] })
  | (DefinitionBase & { kind: 'enum'; values: string[] })
  | (DefinitionBase & { kind: 'typedef'; aliased_type: string })
  | (DefinitionBase & { kind: 'callback'; return_type: string; arguments: Argument[] })

export interface ManifestVersion {
  version: string
  date: string
  commit: string
}

export interface Manifest {
  engines: Record<string, ManifestVersion[]>
}

export interface SnapshotSource {
  repo: string
  tag: string
  commit: string
}

export interface Snapshot {
  engine: string
  version: string
  source: SnapshotSource
  date: string
  entries: Record<string, string>
  parse_errors?: string[]
}

/** One child of a `SearchEntry`: a member/field/value/argument, reduced to
 * just enough to search and to resolve its type to another definition. */
export interface SearchChild {
  name: string
  type: string
}

/** A compact per-definition projection served as `<version>.index.json`,
 * used only for search and the "explore from a global" navigation. */
export interface SearchEntry {
  name: string
  kind: string
  extended_attributes: string[]
  children: SearchChild[]
}
