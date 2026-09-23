// A highlight.js language definition for Locus, for anyone who already uses
// highlight.js: hljs.registerLanguage('locus', locus) and then
// hljs.highlight(source, { language: 'locus' }). Kept deliberately simple;
// editors/highlight/locus.js is the tokenizer that knows the language better.
export default function locus(hljs) {
  const KEYWORDS = {
    keyword: 'fn const let if else struct enum match loop for in break continue while return mut as impl pub prop logic forall exists',
    literal: 'true false',
    type: 'bool u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize Int Nat Bool Prop Seq Map Ghost Option Result Vec Box',
    built_in: 'prove! prop! rewrite! unfold! fold! old! snapshot! recurse! assert! debug_assert! unreachable! todo! panic! matches! vec!',
    $pattern: /[A-Za-z_][A-Za-z0-9_]*!?/
  };
  return {
    name: 'Locus',
    aliases: ['lc'],
    keywords: KEYWORDS,
    contains: [
      { className: 'meta', begin: /\/\/~/, end: /$/ },
      hljs.COMMENT(/\/\//, /$/),
      { className: 'meta', begin: /#!?\[/, end: /\]/, contains: [hljs.QUOTE_STRING_MODE] },
      hljs.QUOTE_STRING_MODE,
      { className: 'number', begin: /\b(0x[0-9A-Fa-f_]+|0o[0-7_]+|0b[01_]+|[0-9][0-9_]*)(u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)?\b/ },
      { className: 'symbol', begin: /@[A-Za-z_][A-Za-z0-9_:]*/ },
      { className: 'symbol', begin: /@/ },
      { className: 'title.function_', begin: /\bfn\s+/, end: /[A-Za-z_][A-Za-z0-9_]*/, excludeBegin: true, keywords: 'fn' },
      { className: 'title.class_', begin: /\b(prop|struct|enum|impl)\s+/, end: /[A-Za-z_][A-Za-z0-9_]*/, excludeBegin: true, keywords: 'prop struct enum impl' },
      { className: 'title.class_', begin: /\b[A-Z][A-Za-z0-9_]*\b/ },
      { className: 'literal', begin: /(?<![A-Za-z0-9_])_(?![A-Za-z0-9_])/ },
      { className: 'operator', begin: /=>|->|==|!=|<=|>=|&&|\|\|/ }
    ]
  };
}
