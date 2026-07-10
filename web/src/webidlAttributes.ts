// Extended attribute names defined by the WebIDL and HTML specifications,
// as opposed to browser-engine-internal annotations (Gecko's
// `Pref`/`ChromeOnly`/`Throws`/`Pure`, Blink's `RuntimeEnabled`/`MeasureAs`,
// etc.). Used to filter engine-internal noise out of cross-engine/webref
// diffs, where those annotations otherwise dominate the changed-lines count
// without reflecting an actual API difference.
//
// Not authoritative -- there's no machine-readable registry of these, so
// this is the spec's extended-attributes vocabulary plus a few pre-rename
// spellings (e.g. `NoInterfaceObject` before it became
// `LegacyNoInterfaceObject`) that still show up in engine IDL sources.
const STANDARD_EXTENDED_ATTRIBUTES = new Set([
  // https://webidl.spec.whatwg.org/#idl-extended-attributes
  'AllowShared',
  'Clamp',
  'CrossOriginIsolated',
  'Default',
  'EnforceRange',
  'Exposed',
  'Global',
  'LegacyFactoryFunction',
  'LegacyLenientSetter',
  'LegacyLenientThis',
  'LegacyNamespace',
  'LegacyNoInterfaceObject',
  'LegacyNullToEmptyString',
  'LegacyOverrideBuiltIns',
  'LegacyTreatNonObjectAsNull',
  'LegacyUnenumerableNamedProperties',
  'LegacyUnforgeable',
  'LegacyWindowAlias',
  'NewObject',
  'PutForwards',
  'Replaceable',
  'SameObject',
  'SecureContext',
  'Unscopable',
  // https://html.spec.whatwg.org/multipage/dom.html#htmlconstructor
  'CEReactions',
  'HTMLConstructor',
  // pre-rename spellings still found in engine IDL sources
  'Constructor',
  'NamedConstructor',
  'NoInterfaceObject',
  'OverrideBuiltins',
  'TreatNonObjectAsNull',
  'Unforgeable',
  'WindowAlias',
  'LenientThis',
  'LenientSetter',
])

function attributeName(attr: string): string {
  return /^[A-Za-z_][A-Za-z0-9_]*/.exec(attr)?.[0] ?? attr
}

export function isStandardExtendedAttribute(attr: string): boolean {
  return STANDARD_EXTENDED_ATTRIBUTES.has(attributeName(attr))
}
