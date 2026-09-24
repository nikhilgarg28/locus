# Locus website

The public website is a static book generated from Markdown. There is no browser-side Markdown parser, application server, or write API. Reading, navigation, source links, and test disclosures work without JavaScript; search, copying, filters, and legacy bookmark redirects use small local scripts.

## Build and preview

Requires Python 3.11+, Node 20+, and npm 10+ (CI uses Python 3.12 and Node 22).

```sh
npm ci --prefix website
python3 tools/site.py build
python3 tools/site.py serve --port 8765 --no-open
python3 tools/site.py check
python3 tools/test_site.py
```

Open the printed local URL. Rebuild after editing Markdown, then refresh. Output is `target/site/`, ignored by Git. It can be served at a domain root or under a path such as `/locus/`, or opened as local HTML. `--out DIRECTORY` supports a separate preview; the builder refuses to replace a nonempty directory it does not own. A failed build leaves the previous preview intact.

## Where content lives

| Source                                        | Public destination                               |
| --------------------------------------------- | ------------------------------------------------ |
| `docs/home.md`                                | Home                                             |
| `docs/specification.md`, `docs/spec/*.md`     | Ordered language manual                          |
| `docs/reference/*.md`                         | Kernel contract and formal core                  |
| `docs/examples.md`                            | Examples                                         |
| `docs/correctness.md`, `docs/architecture.md` | Correctness and implementation                   |
| `docs/performance.md`                         | Performance, with recorded data                  |
| `docs/roadmap.md`, `docs/roadmap/*.md`        | Projects and permanent LOC tasks                 |
| `docs/overview.md`, `docs/development.md`     | Getting started and contributing                 |
| `docs/data/state.json`                        | Measurement receipts and retained Atlas metadata |

Each page has TOML front matter between `+++` delimiters, with a unique `id`, `title`, `route`, and `order`. `group = "Now"` publishes a page. Historical plans, archived decisions, and future vision remain in the repository without becoming current language documentation. Project files use `kind = "project"` and contain tasks below `## LOC-N · Title`, with a small metadata comment and a stable HTML anchor for GitHub navigation.

`tools/content.py` loads this structure. `tools/site_data.py` checks specification citations and prepares test excerpts, benchmark records, and freshness-aware measurements. `build.mjs` uses the pinned **marked** parser, the repository's Locus highlighter, and templates/styles in this directory to emit complete HTML. Output is staged and validated before it replaces the previous build.

Use real relative Markdown links. The generator maps published Markdown to its HTML route, and repository source files to GitHub. Rule markers (`<!-- spec: 1.2:3 legality-rule -->`) produce stable anchors and native expandable test lists. `<!-- component: name -->` inserts repository-derived views such as benchmark charts; prose stays in Markdown.

## Checked examples and excerpts

Every executable example in a current page is a complete program in a `rust check`, `rust run`, or `rust reject Lxxxx` fence. `run` examples include `//~ run:` expectations. Only grammar, shell commands, and other honestly non-executable illustrations use `LANG prose REASON`.

The `rust` language label provides GitHub syntax highlighting. The `check`, `run`, and `reject` modes still select the Locus compiler; the website displays these as Locus with its own highlighter. Actual Rust illustrations use `rust prose REASON` on current pages. Legacy `locus` and `lc` fences remain accepted by the checker, but new Markdown should use `rust`. Standalone `.lc` files use the same GitHub fallback through the root `.gitattributes`.

Keep helpers and test expectations in the same Markdown fence. Surround lines that would distract from the lesson with `// docs:hide` and `// docs:show`:

````markdown
```rust run
// docs:hide
fn helper() -> u8 { 41 }
// docs:show
fn answer() -> u8 { helper().wrapping_add(1) }
// docs:hide
//~ run: answer() => 42
// docs:show
```
````

The page initially shows `answer` and labels it an excerpt. A native **Complete checked example** disclosure includes the helper and run expectation, with its own copy button. Both views omit the marker comments. It works without JavaScript; copying requires JavaScript.

The compiler always receives the full original fence, including hidden lines. Markers must be whole-line comments, balanced within one checked fence, and cannot nest. Unknown markers, an unclosed hidden region, or a wholly hidden example fail the build. Use hidden regions for setup and test drivers; keep assumptions, proof obligations, and the steps being taught visible. An excerpt is not a separate untested program.

## Validation and publication

`tools/site.py check` builds and rejects broken local files/fragments, duplicate IDs, missing rule anchors, or operative rules without focused tests. `tools/test_site.py` checks canonical round-tripping, invalid metadata, task preservation, rule/test rendering, and path-prefix navigation. The regular `tools/check.sh` gate includes both. Executable Markdown examples run in `tests/atlas_fences.rs`; the historical test name is retained. The website workflow installs Rust 1.91.1 and runs that harness and its adversarial gate tests before uploading the Pages artifact.

GitHub Actions builds, checks, and executes the current-language examples on every branch push and pull request before creating a Pages artifact. Deployment is separate: enable **Settings → Pages → GitHub Actions**, then run the Website workflow on `main` with **Publish** selected. Ordinary pushes do not deploy. The artifact is also usable with any static host. [GitHub's workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) describes the Pages setup.

Benchmark data comes from the `locus-bench-data` branch (local or `origin`); a full clone fetches it. Missing data produces an explicit empty state. Historical timings are labeled with revision, date, and dirty status; stale measurements never become current test counts. No network requests or fabricated samples are needed during generation.

## Design references and migration

The organization follows [Rue's public site](https://rue-lang.dev/) and its [repository](https://github.com/rue-language/rue/tree/d39685970f97ae94258d75bdc44a0e5a710f9713): numbered Markdown chapters, separately maintained prose, generated traceability, shared templates, and a validated build artifact. Rue's production build uses its Gazette generator. Locus uses a small Node/Python build to fit its existing tooling. The warm paper, serif reading typography, restrained navigation, and book-like rhythm also take inspiration from [Crafting Interpreters](https://craftinginterpreters.com/). The layout and assets here are original.

The Atlas migration preserves all 341 original rule IDs and all 232 public task IDs. The later manual rewrite adds type-by-type explanations, shorter rules, and checked teaching examples while retaining the original IDs. Additional split rules have their own focused citations. A code-backed audit consolidated 23 projects into nine and corrected 79 task statuses, leaving partial or deferred work open. Completed work cites implementation/tests in its task notes.

Root `atlas.html` points at the local built site; the artifact's own `atlas.html` resolves old rule and task bookmarks. The old Atlas accidentally assigned internal ID `t179` to both LOC-85 and LOC-86; that ambiguous bookmark opens the roadmap index rather than choosing a task. Public LOC links remain unambiguous. HTML is no longer a source of truth; edit Markdown directly or use the compatibility `tools/atlas.py` commands.
