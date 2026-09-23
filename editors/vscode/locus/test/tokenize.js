// Runs the TextMate grammar over every .lc file in the repository with the
// same engine VS Code uses (vscode-textmate over Oniguruma), and checks that
// the scopes a reader relies on are assigned: keywords, forms, evidence,
// attributes, directives, declarations. Needs `npm install` in this folder
// once (vscode-textmate, vscode-oniguruma); tools/check.sh runs it when both
// are present. Run: node editors/vscode/locus/test/tokenize.js
const fs = require('fs');
const path = require('path');
let vsctm, oniguruma;
try { vsctm = require('vscode-textmate'); oniguruma = require('vscode-oniguruma'); }
catch (e) { console.log('skipped: vscode-textmate and vscode-oniguruma are not installed (npm install in editors/vscode/locus)'); process.exit(0); }

const here = __dirname;
const root = path.resolve(here, '..', '..', '..', '..');
const grammarPath = path.join(here, '..', 'syntaxes', 'locus.tmLanguage.json');
const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm')).buffer;
const onig = oniguruma.loadWASM(wasm).then(() => ({
  createOnigScanner: (s) => new oniguruma.OnigScanner(s),
  createOnigString: (s) => new oniguruma.OnigString(s),
}));
const registry = new vsctm.Registry({
  onigLib: onig,
  loadGrammar: (scope) => scope === 'source.locus' ? Promise.resolve(vsctm.parseRawGrammar(fs.readFileSync(grammarPath, 'utf8'), grammarPath)) : Promise.resolve(null),
});

const files = [];
(function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) { if (entry.name !== 'target' && entry.name !== 'node_modules' && !entry.name.startsWith('.')) walk(p); }
    else if (entry.name.endsWith('.lc')) files.push(p);
  }
})(root);

registry.loadGrammar('source.locus').then((grammar) => {
  let failures = 0, tokens = 0;
  const seen = new Map();
  const want = {
    'keyword.other.fn.locus': 0, 'entity.name.function.locus': 0, 'support.function.form.locus': 0,
    'keyword.operator.evidence.locus': 0, 'entity.name.type.proposition.locus': 0, 'meta.attribute.locus': 0,
    'entity.name.attribute.locus': 0, 'comment.line.directive.locus': 0, 'keyword.other.directive.locus': 0,
    'keyword.control.locus': 0, 'storage.type.primitive.locus': 0, 'constant.numeric.integer.locus': 0,
    'string.quoted.double.locus': 0, 'keyword.operator.hole.locus': 0, 'keyword.operator.implication.locus': 0,
    'entity.name.type.locus': 0, 'keyword.other.reserved.locus': 0,
  };
  for (const file of files) {
    const lines = fs.readFileSync(file, 'utf8').split('\n');
    let state = vsctm.INITIAL;
    for (const line of lines) {
      const r = grammar.tokenizeLine(line, state);
      for (const t of r.tokens) {
        tokens++;
        if (t.scopes.length < 1 || t.scopes[0] !== 'source.locus') { failures++; console.error('bad scopes in ' + file + ': ' + JSON.stringify(t)); }
        for (const s of t.scopes) { seen.set(s, (seen.get(s) || 0) + 1); if (s in want) want[s]++; }
        const text = line.slice(t.startIndex, t.endIndex);
        if (/^\s*$/.test(text)) continue;
        if (t.scopes.length === 1 && /^(fn|let|match|prove!|prop!|@)$/.test(text.trim())) { failures++; console.error('unscoped ' + JSON.stringify(text) + ' in ' + path.relative(root, file)); }
      }
      state = r.ruleStack;
    }
  }
  for (const [scope, count] of Object.entries(want)) if (!count) { failures++; console.error('scope never assigned: ' + scope); }
  const top = [...seen.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([s, c]) => c + ' ' + s).join('\n  ');
  console.log((failures ? 'FAILED ' : 'ok ') + files.length + ' files, ' + tokens + ' tokens; most common scopes:\n  ' + top);
  process.exit(failures ? 1 : 0);
}).catch((e) => { console.error(e); process.exit(1); });
