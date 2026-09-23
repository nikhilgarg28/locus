# Locus highlighting in the browser

Two modules, for two situations.

- `locus.js`: a tokenizer with no dependencies. `highlightLocus(source)` returns HTML whose spans carry highlight.js class names (`hljs-keyword`, `hljs-built_in` for the forms, `hljs-symbol` for `@`, `hljs-meta` for attributes and `//~` directives), so any highlight.js theme colours it, and the output with tags removed is the source, exactly. Load it as a classic script (`window.highlightLocus`), require it from Node, or import it for its side effect from an ES module. The static website imports this file directly in `website/build.mjs`; rebuilding the site uses the current tokenizer. `python3 tools/highlight.py check` runs the Node tokenizer tests for the compiler gate. The former `put` command is a deprecated no-op: there is no embedded copy to synchronize.
- `locus.hljs.js`: a language definition for highlight.js, for pages that already use it: `hljs.registerLanguage('locus', locus)`.

For Shiki, Monaco, or anything else that reads a TextMate grammar, use `editors/vscode/locus/syntaxes/locus.tmLanguage.json` directly; it is the same grammar VS Code uses.

`demo.html` shows both modules on one example; `node test.js` checks the tokenizer over every `.lc` file in the repository.
