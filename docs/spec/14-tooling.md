+++
id = "language-tooling"
title = "Checking and diagnostics"
group = "Now"
spec_chapter = 1
order = 113
route = "specification/tooling.html"
description = "Proof lockfiles, diagnostics, and the compiler commands."
+++

# Checking and diagnostics

<!-- spec: 1.0:15 informative -->
The checking workflow combines diagnostics, explicit proof steps and stored certificates. This chapter explains how to reuse checked proofs, investigate an unmet obligation and control the compiler from the command line.

## Found proofs are stored

<!-- spec: 1.20:1 legality-rule -->
`locus check file.lc` stores found proofs in `Locus.lock` in the source directory. Version 2 is TOML: each `[[file]]` table names a source by its relative path with forward slashes, and each obligation records its key, function/ordinal label, claimed conclusion, and named proof steps. The key hashes the claim and its context as kernel terms with stable declaration names and numbered context positions; it finds a candidate and grants no authority. Every candidate is parsed and checked against the current obligation, so a stale or edited entry costs a search and never passes a false claim. `--locked` searches for nothing and never writes or migrates storage; a missing or stale entry is an error naming the function, line, and claim. `--no-store`, or `LOCUS_PROOFS=off`, neither reads nor writes storage; `LOCUS_SEARCH=none` disables every tier, under which a complete lockfile still checks. A successful unlocked check replaces only the checked source's entries with the canonical entries used during that run, drops unused entries for that source, and preserves other source tables. Files are written in path order and obligations in encounter order. If no table exists for a source but its version-1 `<source>.proofs` sidecar exists, checking may read it; only after a successful unlocked check writes its entries into `Locus.lock` is that sidecar deleted. A lockfile table takes precedence when both forms exist. Examples and target files have committed directory lockfiles. Identical inputs produce identical canonical proofs, diagnostics, and Rust; changing the source filename changes its table path, while reformatting and local renaming do not change obligation keys.

## Diagnostics

<!-- spec: 1.21:1 legality-rule -->
Every diagnostic has a stable code, source spans, explanatory notes and, where appropriate, a suggested edit. Lexer, parser, elaborator and driver errors have distinct code ranges. The corpus metadata test requires coverage for every reachable code, including preview, model, scope, borrowing and trust-boundary errors. Errors in source exit with status 1; command-line errors exit with 2. `locus check --error-format json` emits one one-element JSON array per diagnostic line on stderr, using fixed diagnostic schema 1. All optional proof fields are present and null when unavailable. The envelope includes severity, code, message, spans (exactly one primary), notes, helps and suggestions, plus the claim, claim after computing, facts considered, counterexample and checked explicit proof form. Source positions are mapped to the original entry/library file before stable sorting by file name and byte offset. Changing the fixed keys, field types or field meanings requires a schema-version increment. `locus explain CODE` prints a code-specific cause, example and stable Language paragraph reference. The complete field contract and migration history are in `docs/diagnostics/schema.md`; every source rejection and driver/resource diagnostic has a JSON golden, and every code has an explanation golden.

<!-- spec: 1.21:2 legality-rule -->
A hole that cannot be filled, `L0230`, and an operator whose obligation cannot be met, `L0235`, report the claim, the claim after computing when that differs or the bound the operator needs, the facts considered, at most six, and the counterexample the arithmetic procedure found, which it has checked against the facts it collected: `it fails when hi = 0, lo = 1, which the arithmetic facts known here allow`. When a definition, an equation, a connective, or a lemma is what is missing, the note names the form that takes the step. A use of stale evidence names the assignment that invalidated it and the claim now owed. A parser error carries a fix where one is mechanical, and parsing goes on so that every error in a file is reported; the parser is total, by a step bound and a depth bound tested on their own.

## The command line

<!-- spec: 1.22:1 example -->
~~~text prose shell-commands
locus check file.lc [--holes] [--stats] [--locked] [--no-store]
locus run file.lc function [bool | integer]...
locus rust file.lc
locus audit file.lc
locus build file.lc... --out dir [--name crate]
locus tokens file.lc
locus parse file.lc
locus ast file.lc
~~~

<!-- spec: 1.22:2 legality-rule -->
`--library path.lc` includes ordinary checked library declarations, with diagnostics mapped to the original files. Preview names are owned by tasks in a closed registry. All current features are stabilized; passing one of their obsolete flags is an error that asks the caller to remove it.

<!-- spec: 1.22:3 legality-rule -->
`run` takes `bool` and machine-integer arguments on the command line. A run line in a corpus file passes any value, evidence as `Erased`, and `&mut` arguments with the value they hold afterwards.
