// A syntax highlighter for Locus with no dependencies: highlightLocus(source)
// returns HTML in which every token is wrapped in a span whose class is one
// of highlight.js's (hljs-keyword, hljs-string, ...), so that any highlight.js
// theme colours it, and text outside spans is the source, escaped. The
// output with tags removed and entities unescaped is the source, exactly:
// editors/highlight/test.js checks that over every .lc file in the repository.
//
// Usable three ways: as a classic script (defines window.highlightLocus), as a
// CommonJS module (module.exports), and from an ES module by importing this
// file for its side effect and reading globalThis.highlightLocus. The same
// module is imported directly by website/build.mjs; no embedded copy exists.
(function (root) {
  'use strict';

  var KEYWORDS = new Set(['fn', 'const', 'let', 'if', 'else', 'struct', 'enum', 'match', 'loop', 'for', 'in',
    'break', 'continue', 'while', 'return', 'mut', 'as', 'impl', 'pub', 'prop', 'logic', 'spec', 'import', 'trait']);
  var QUANTIFIERS = new Set(['forall', 'exists']);
  var RESERVED = new Set(['async', 'await', 'crate', 'dyn', 'extern', 'mod', 'move', 'ref', 'static', 'super',
    'type', 'unsafe', 'use', 'where', 'abstract', 'become', 'box', 'do', 'final', 'gen', 'macro', 'override', 'priv',
    'try', 'typeof', 'unsized', 'virtual', 'yield']);
  var LITERALS = new Set(['true', 'false', '_']);
  var SELF = new Set(['self', 'Self']);
  var PRIMITIVES = new Set(['bool', 'u8', 'u16', 'u32', 'u64', 'u128', 'usize', 'i8', 'i16', 'i32', 'i64', 'i128', 'isize']);
  var LOGIC_TYPES = new Set(['Int', 'Nat', 'Bool', 'Prop', 'Seq', 'Map', 'Ghost', 'Option', 'Result', 'Vec', 'Box', 'Model', 'Logical']);
  var FORMS = new Set(['prove', 'prop', 'rewrite', 'unfold', 'fold', 'old', 'snapshot', 'model', 'recurse', 'assert',
    'debug_assert', 'unreachable', 'todo', 'panic', 'matches', 'vec']);
  var ATTRIBUTES = new Set(['terminates', 'no_panic', 'no_alloc', 'no_io', 'derive', 'decreases']);
  var SUFFIX = /^(u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)\b/;
  var OPERATORS = ['..=', '..', '=>', '->', '==', '!=', '<=', '>=', '&&', '||', '::', '+', '-', '*', '/', '%', '=', '<', '>', '!', '&', '@'];

  function esc(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }
  function span(cls, text) {
    return '<span class="' + cls + '">' + esc(text) + '</span>';
  }
  function isNameStart(c) { return /[A-Za-z_]/.test(c); }
  function isNameChar(c) { return /[A-Za-z0-9_]/.test(c); }

  function highlightLocus(src) {
    var out = [];
    var i = 0, n = src.length;
    var afterFn = false, afterDecl = false, afterAt = false, inAttribute = 0;
    while (i < n) {
      var c = src[i];
      // comments: directives, docs, plain
      if (c === '/' && src[i + 1] === '/') {
        var end = src.indexOf('\n', i); if (end < 0) end = n;
        var text = src.slice(i, end);
        if (text.startsWith('//~')) {
          var m = /^(\/\/~\^*\s*)([a-zA-Z-]+:?)?/.exec(text);
          var head = m[0];
          out.push(span('hljs-meta', head) + span('hljs-comment', text.slice(head.length)));
        } else {
          out.push(span('hljs-comment', text));
        }
        i = end; continue;
      }
      // attributes: #[...] and #![...]
      if (c === '#' && (src[i + 1] === '[' || (src[i + 1] === '!' && src[i + 2] === '['))) {
        var open = src[i + 1] === '[' ? 2 : 3;
        out.push(span('hljs-meta', src.slice(i, i + open)));
        i += open; inAttribute = 1; continue;
      }
      if (inAttribute) {
        if (c === '[') { inAttribute++; out.push(span('hljs-meta', c)); i++; continue; }
        if (c === ']') { inAttribute--; out.push(span('hljs-meta', c)); i++; continue; }
      }
      // strings
      if (c === '"') {
        var j = i + 1;
        while (j < n && src[j] !== '"') { if (src[j] === '\\') j++; j++; }
        j = Math.min(j + 1, n);
        out.push(span('hljs-string', src.slice(i, j)));
        i = j; continue;
      }
      // numbers, with their suffix
      if (/[0-9]/.test(c)) {
        var j2 = i + 1;
        if (c === '0' && /[xob]/.test(src[i + 1])) { j2 = i + 2; while (j2 < n && /[0-9A-Fa-f_]/.test(src[j2])) j2++; }
        else while (j2 < n && /[0-9_]/.test(src[j2])) j2++;
        var rest = src.slice(j2), sm = SUFFIX.exec(rest);
        var num = src.slice(i, j2);
        out.push(span('hljs-number', num) + (sm ? span('hljs-type', sm[1]) : ''));
        i = j2 + (sm ? sm[1].length : 0); continue;
      }
      // names, keywords, forms, types
      if (isNameStart(c)) {
        var k = i + 1; while (k < n && isNameChar(src[k])) k++;
        var word = src.slice(i, k);
        var next = src.slice(k).replace(/^\s*/, '')[0];
        var cls;
        if (src[k] === '!' && FORMS.has(word) && /[(\[{]/.test(src.slice(k + 1).replace(/^\s*/, '')[0] || '')) {
          out.push(span('hljs-built_in', word + '!')); i = k + 1; afterFn = afterDecl = afterAt = false; continue;
        }
        if (afterAt) cls = 'hljs-symbol';
        else if (afterFn) cls = 'hljs-title function_';
        else if (afterDecl) cls = 'hljs-title class_';
        else if (inAttribute && ATTRIBUTES.has(word)) cls = 'hljs-meta';
        else if (KEYWORDS.has(word)) cls = 'hljs-keyword';
        else if (QUANTIFIERS.has(word) && next === '(') cls = 'hljs-keyword';
        else if (RESERVED.has(word) && /pub\s*\(\s*(in\s+)?$/.test(src.slice(Math.max(0, i - 12), i))) cls = 'hljs-keyword';
        else if (RESERVED.has(word)) cls = 'hljs-keyword locus-reserved';
        else if (LITERALS.has(word)) cls = 'hljs-literal';
        else if (SELF.has(word)) cls = 'hljs-variable language_';
        else if (PRIMITIVES.has(word) || LOGIC_TYPES.has(word)) cls = 'hljs-type';
        else if (/^[A-Z]/.test(word)) cls = 'hljs-title class_';
        else cls = '';
        out.push(cls ? span(cls, word) : esc(word));
        afterAt = false;
        afterFn = word === 'fn';
        afterDecl = word === 'prop' || word === 'struct' || word === 'enum' || word === 'impl';
        i = k; continue;
      }
      // the hole
      if (c === '_' ) { out.push(span('hljs-literal', c)); i++; continue; }
      // operators, including @ for evidence
      var op = null;
      for (var o = 0; o < OPERATORS.length; o++) if (src.startsWith(OPERATORS[o], i)) { op = OPERATORS[o]; break; }
      if (op) {
        if (op === '@') { out.push(span('hljs-symbol', op)); afterAt = true; }
        else if (op === '::' || op === '..' || op === '..=') { out.push(esc(op)); }
        else out.push(span('hljs-operator', op));
        i += op.length; continue;
      }
      // whitespace and punctuation pass through, escaped
      var p = i + 1;
      while (p < n && /[\s(){}\[\];,.:]/.test(src[p]) && src[p] !== ':') p++;
      out.push(esc(src.slice(i, p)));
      if (!/\s/.test(src.slice(i, p))) afterFn = afterDecl = false;
      i = p;
    }
    return out.join('');
  }

  highlightLocus.version = 1;
  root.highlightLocus = highlightLocus;
  if (typeof module !== 'undefined' && module.exports) module.exports = highlightLocus;
})(typeof globalThis !== 'undefined' ? globalThis : this);
