# Locus for Visual Studio Code

Syntax highlighting for `.lc` files, and for ```` ```locus ```` fences in Markdown.

The grammar (`syntaxes/locus.tmLanguage.json`) is a TextMate grammar, so it also works anywhere else that reads one: Shiki in the browser, GitHub Linguist, Zed, Sublime Text. The scopes it assigns:

| Scope | What |
|---|---|
| `keyword.control.locus`, `keyword.other.locus` | `if`, `match`, `loop`, `fn`, `let`, `mut`, `prop`, `logic`, `as`, ... |
| `keyword.other.quantifier.locus` | `forall`, `exists` before `(` |
| `keyword.other.reserved.locus` | every Rust keyword Locus reserves and refuses |
| `keyword.operator.evidence.locus` | `@`, both in `@P` and in `Pred::Arm(w) @ evidence` |
| `entity.name.type.proposition.locus` | the name after `@` |
| `keyword.operator.hole.locus` | `_` as a request for evidence |
| `support.function.form.locus` | the built-in forms `prove!`, `prop!`, `rewrite!`, `unfold!`, `fold!`, ... |
| `meta.attribute.locus`, `entity.name.attribute.locus` | `#[terminates]`, `#[no_panic]`, `#[no_io]`, `#[no_alloc]`, `#[derive(...)]`, `#![...]` |
| `comment.line.directive.locus`, `keyword.other.directive.locus` | the corpus directives `//~ run:`, `//~ proofs:`, `//~ error:`, ... |
| `storage.type.primitive.locus`, `support.type.logic.locus` | the machine types; `Int`, `Nat`, `Bool`, `Prop`, `Seq`, `Map` |
| `keyword.operator.implication.locus` | `=>` |

## Install

From this repository, without packaging: link the folder into the extensions directory and reload the window.

```bash
ln -s "$(pwd)/editors/vscode/locus" ~/.vscode/extensions/locus-lang.locus-language-0.1.0
```

Or package it with `npx @vscode/vsce package` in this folder and install the `.vsix` with `code --install-extension`.

The grammar is tested against every `.lc` file in the repository by `editors/vscode/locus/test/tokenize.js` (needs `npm install` in that folder once); `tools/check.sh` runs it when Node is present.
