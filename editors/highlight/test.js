// Highlighting is lossless: over every .lc file in the repository, the HTML
// with its tags removed and its entities unescaped is the source. A few
// classes are asserted on a known file so that a regression in the tokenizer
// is visible. Run with: node editors/highlight/test.js
const fs = require('fs');
const path = require('path');
const highlightLocus = require('./locus.js');

const root = path.resolve(__dirname, '..', '..');
const files = [];
(function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) { if (entry.name !== 'target' && entry.name !== 'node_modules' && !entry.name.startsWith('.')) walk(p); }
    else if (entry.name.endsWith('.lc')) files.push(p);
  }
})(root);

function untag(html) {
  return html.replace(/<[^>]+>/g, '').replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}

let failures = 0;
for (const file of files) {
  const src = fs.readFileSync(file, 'utf8');
  const html = highlightLocus(src);
  if (untag(html) !== src) {
    failures++;
    console.error('not lossless: ' + path.relative(root, file));
  }
  if (/<span class="[^"]*"><\/span>/.test(html)) { failures++; console.error('empty span in ' + path.relative(root, file)); }
}
const lock = highlightLocus(fs.readFileSync(path.join(root, 'examples', 'lock.lc'), 'utf8'));
const expect = [
  ['hljs-keyword', 'fn'], ['hljs-meta', '#['], ['hljs-meta', 'derive'], ['hljs-built_in', 'prop!'], ['hljs-built_in', 'prove!'],
  ['hljs-symbol', '@'], ['hljs-symbol', 'within_limit'], ['hljs-title class_', 'Lock'], ['hljs-title function_', 'step'],
  ['hljs-type', 'u8'], ['hljs-type', 'Prop'], ['hljs-number', '3'], ['hljs-literal', 'true'], ['hljs-comment', '// A lock that tolerates three wrong codes. The failure count never exceeds'],
];
for (const [cls, text] of expect) {
  const needle = '<span class="' + cls + '">' + text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;') + '</span>';
  if (!lock.includes(needle)) { failures++; console.error('expected ' + needle + ' in lock.lc'); }
}
const directive = highlightLocus('//~ run: step(3) => 4\n');
if (!directive.includes('<span class="hljs-meta">//~ run:</span>')) { failures++; console.error('directive head not marked: ' + directive); }
console.log((failures ? 'FAILED ' : 'ok ') + files.length + ' files highlighted losslessly' + (failures ? ', ' + failures + ' failures' : ''));
process.exit(failures ? 1 : 0);
