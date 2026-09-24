/** Markdown is the source; this compiler emits complete, link-checked static HTML. */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { marked, Renderer } from "marked";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const highlightLocus = require("../editors/highlight/locus.js");
const args = process.argv.slice(2);
const arg = (name) =>
  args.includes(name) ? args[args.indexOf(name) + 1] : null;
const output = path.resolve(arg("--out") || path.join(ROOT, "target/site"));
if (
  output === ROOT ||
  ROOT.startsWith(output + path.sep) ||
  (fs.existsSync(output) &&
    fs.readdirSync(output).length &&
    !fs.existsSync(path.join(output, ".locus-site")))
) {
  throw Error(
    "Refusing to replace a directory not owned by the site build: " + output,
  );
}
const python = process.env.PYTHON || "python3";
const prepared = spawnSync(python, [path.join(ROOT, "tools/site_data.py")], {
  cwd: ROOT,
  encoding: "utf8",
  maxBuffer: 32 * 1024 * 1024,
});
if (prepared.status !== 0) {
  process.stderr.write(prepared.stderr);
  process.exit(1);
}
const db = JSON.parse(prepared.stdout);
const esc = (s) =>
  String(s).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
const slug = (s) =>
  String(s)
    .replace(/<[^>]*>/g, "")
    .replace(/[`*]/g, "")
    .toLowerCase()
    .replace(/^\d+\.\s*/, "")
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-|-$/g, "");
// Search raw Markdown without dropping generic type arguments as HTML tags.
const plain = (s) =>
  String(s)
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/[`*#]/g, "")
    .replace(/\s+/g, " ")
    .trim();
const plainHTML = (s) => plain(String(s).replace(/<[^>]*>/g, ""));
const docs = db.docs.filter((d) => d.publish);
const chapters = docs
  .filter((d) => d.spec_chapter === 1)
  .sort((a, b) => a.order - b.order);
const byId = new Map(db.docs.map((d) => [d.id, d]));
const linkable = [
  ...db.docs,
  ...db.projects.map((p) => ({ ...p, publish: true })),
];
const bySource = new Map(linkable.map((d) => [d.source, d]));
const byRule = new Map(db.rules.map((r) => [r.id, r]));
const fenceKey = (doc, source) => `${doc}\0${source.trimEnd()}`;
const checkedFences = new Map(
  db.fences.map((f) => [fenceKey(f.doc, f.code), f]),
);
const tasks = new Map(db.tasks.map((t) => [t.n, t]));
const projects = new Map(db.projects.map((p) => [p.id, p]));
const ruleUses = new Map();
for (const c of db.citations)
  for (const id of c.ids) {
    if (!ruleUses.has(id)) ruleUses.set(id, []);
    ruleUses.get(id).push(c);
  }
const relative = (route, target) =>
  path.posix.relative(path.posix.dirname(route), target) ||
  path.posix.basename(target);
const sourceURL = (file, line) =>
  `${db.repository}/blob/${db.revision}/${file}${line ? "#L" + line : ""}`;
const taskTarget = (n) => {
  const t = tasks.get(Number(n));
  return t ? `${projects.get(t.project).route}#LOC-${t.n}` : null;
};
const ruleTarget = (id) => {
  const r = byRule.get(id);
  return r ? `${byId.get(r.doc).route}#spec-${id}` : null;
};
const NAV = [
  ["specification", "Specification", "specification/index.html"],
  ["examples", "Examples", "examples.html"],
  ["correctness", "Correctness", "correctness.html"],
  ["performance", "Performance", "performance.html"],
  ["roadmap", "Roadmap", "roadmap.html"],
];
const searches = [];
const outputFiles = new Map();
function resolveLink(href, doc) {
  if (!href) return "#";
  if (/^(https?:|mailto:|tel:|#)/.test(href)) return href;
  if (/^[a-z]+:/i.test(href))
    throw Error(`Unsafe link ${href} in ${doc.source}`);
  const old = /atlas\.html#\/doc\/([^/]+)(?:\/(.*))?/.exec(href);
  if (old) {
    const target = old[2]?.startsWith("spec-")
      ? ruleTarget(old[2].slice(5))
      : byId.get(old[1])?.route;
    return target ? relative(doc.route, target) : sourceURL("atlas.html");
  }
  const [pathname, hash] = href.split("#");
  const candidate = path.posix.normalize(
    path.posix.join(path.posix.dirname(doc.source), pathname),
  );
  const bare = path.posix.basename(pathname).replace(/\.(md|html)$/, "");
  const found =
    bySource.get(candidate) ||
    byId.get(bare) ||
    linkable.find(
      (d) => d.id === bare || path.posix.basename(d.source, ".md") === bare,
    ) ||
    linkable.find((d) => d.route === pathname);
  if (found) {
    if (!found.publish) return sourceURL(found.source);
    return relative(doc.route, found.route) + (hash ? "#" + hash : "");
  }
  if (fs.existsSync(path.join(ROOT, candidate))) return sourceURL(candidate);
  if (fs.existsSync(path.join(ROOT, pathname))) return sourceURL(pathname);
  if (pathname.endsWith(".html"))
    return relative(doc.route, pathname) + (hash ? "#" + hash : "");
  throw Error(`Unresolved link ${href} in ${doc.source}`);
}
function ruleHeader(id, doc) {
  const rule = byRule.get(id);
  if (!rule) throw Error(`Unknown rule ${id}`);
  const uses = ruleUses.get(id) || [];
  const focused = uses.filter((c) => c.focused).length;
  searches.push({
    title: `${id} · ${doc.title}`,
    text: plain(rule.text),
    url: ruleTarget(id),
    kind: "Rule",
  });
  const cases = uses
    .map(
      (c) =>
        `<details class="test-case"><summary><span>${esc(c.test)}</span><small>${c.focused ? "focused" : "context"}</small></summary><div class="test-source"><a href="${sourceURL(c.source, c.line)}">${esc(c.source)}:${c.line} ↗</a><pre><code>${highlightLocus(c.excerpt)}</code></pre>${c.truncated ? '<p class="caption">Excerpt. Follow the source link for the complete test.</p>' : ""}</div></details>`,
    )
    .join("");
  const note = uses.length
    ? `${focused} focused ${focused === 1 ? "test" : "tests"}${focused < uses.length ? ` · ${uses.length - focused} context references` : ""}`
    : ["informative", "example"].includes(rule.category)
      ? "No focused test required for this category."
      : "No linked tests.";
  return `<div class="rule-heading" id="spec-${id}"><a class="rule-id" href="#spec-${id}" aria-label="Link to rule ${id}">${id}</a><span class="rule-category">${esc(rule.category)}</span><details class="rule-tests"><summary>${uses.length ? `${uses.length} ${uses.length === 1 ? "test" : "tests"}` : "About this rule"}<span aria-hidden="true"> +</span></summary><div class="test-list"><p>${note}</p>${cases}</div></details></div>\n`;
}
function markdown(text, doc) {
  const toc = [];
  const ids = new Map();
  const renderer = new Renderer();
  renderer.heading = function ({ tokens, depth }) {
    const title = this.parser.parseInline(tokens);
    const base = slug(title);
    const seen = ids.get(base) || 0;
    ids.set(base, seen + 1);
    const id = (doc.anchorPrefix || "") + base + (seen ? "-" + (seen + 1) : "");
    if (depth > 1) toc.push({ id, title: plainHTML(title), depth });
    return `<h${depth} id="${esc(id)}">${title}${depth > 1 ? `<a class="heading-anchor" href="#${id}" aria-label="Link to ${esc(plainHTML(title))}">#</a>` : ""}</h${depth}>\n`;
  };
  renderer.code = function ({ text, lang }) {
    const [sourceLanguage, mode] = String(lang || "text").split(/\s+/);
    const fence = checkedFences.get(fenceKey(doc.id, text));
    // Rust is the GitHub fallback; checked examples are still Locus programs.
    const language = fence && mode !== "prose" ? "locus" : sourceLanguage;
    if (/^\s*\/\/ docs:/m.test(text) && !fence) {
      throw Error(`Unvalidated example markers in ${doc.source}`);
    }
    const figure = (source, caption, copyLabel) => {
      const highlighted = ["locus", "lc", "rust"].includes(language)
        ? highlightLocus(source)
        : esc(source);
      return `<figure class="code-block"><figcaption><span>${esc(language === "lc" ? "locus" : language)}</span>${caption ? `<span class="code-mode">${esc(caption)}</span>` : ""}<button class="copy-code" type="button" aria-label="${esc(copyLabel)}">Copy</button></figcaption><pre><code class="language-${esc(language)}">${highlighted}</code></pre></figure>\n`;
    };
    const status = mode === "reject" ? "expected rejection" : "checked example";
    if (fence?.excerpt) {
      return `<div class="example-excerpt">${figure(fence.visible_code, `${status} · excerpt`, "Copy excerpt")}<details class="complete-example"><summary>Complete checked example</summary>${figure(fence.complete_code, mode === "run" ? "complete program + run expectations" : "complete program", "Copy complete example")}</details></div>\n`;
    }
    return figure(
      text,
      ["check", "run", "reject"].includes(mode) ? status : "",
      "Copy code",
    );
  };
  renderer.link = function ({ href, title, tokens }) {
    return `<a href="${esc(resolveLink(href, doc))}"${title ? ` title="${esc(title)}"` : ""}>${this.parser.parseInline(tokens)}</a>`;
  };
  renderer.html = function ({ text }) {
    const rule = /^<!--\s*spec:\s*([\d.:]+)(?:\s+[a-z-]+)?\s*-->\s*$/.exec(
      text.trim(),
    );
    if (rule) return ruleHeader(rule[1], doc);
    const component = /^<!-- component: ([a-z-]+) -->$/.exec(text.trim());
    if (component) return components(component[1], doc);
    // Type parameters such as Vec<T> are text, not custom HTML elements.
    // All authored presentation uses Markdown or the named components above.
    return text.trim().startsWith("<!--") ? text : esc(text);
  };
  let html = marked.parse(text, { renderer, gfm: true });
  // Link public task identifiers in prose, never inside code or existing links.
  html = html.replace(
    /(<pre\b[\s\S]*?<\/pre>|<a\b[\s\S]*?<\/a>|<code\b[\s\S]*?<\/code>|<[^>]+>)|\bLOC-(\d+)\b/g,
    (all, tag, n) =>
      tag ||
      (taskTarget(n)
        ? `<a class="task-ref" href="${relative(doc.route, taskTarget(n))}">LOC-${n}</a>`
        : all),
  );
  return { html, toc };
}
function code(source, label = "locus") {
  return `<figure class="code-block"><figcaption><span>${esc(label)}</span><button class="copy-code" type="button" aria-label="Copy code">Copy</button></figcaption><pre><code>${highlightLocus(source)}</code></pre></figure>`;
}
function components(name, doc) {
  const link = (route) => relative(doc.route, route);
  switch (name) {
    case "home":
      return `<div class="hero-actions"><a class="button primary" href="${link("examples.html")}">Start with an example <span>↗</span></a><a class="text-link" href="${link("specification/index.html")}">Read the language manual →</a></div><div class="field-notes"><span class="overline">From the source</span><a href="${link("specification/index.html")}"><strong>${db.coverage.normative}</strong> operative rules</a><a href="${link("correctness.html")}"><strong>${db.coverage.focused_tests}</strong> focused tests cited</a><span><strong>Rust</strong> compilation target</span></div>`;
    case "specimen":
      return `<div class="specimen"><div class="specimen-label"><span class="overline">Example 01</span><span>increment.lc</span></div>${code(db.specimen)}<div class="specimen-notes"><p><b>01 / A result</b><span><code>out: u8</code> is an ordinary byte.</span></p><p><b>02 / A guarantee</b><span><code>@(...)</code> describes the returned evidence.</span></p><p><b>03 / A checked step</b><span>The kernel verifies the proof filling <code>_</code>.</span></p></div></div>`;
    case "specification":
      return `<ol class="chapter-index">${chapters.map((d, i) => `<li><span class="chapter-number">${String(i + 1).padStart(2, "0")}</span><div><a href="${link(d.route)}">${esc(d.title)}</a><p>${esc(d.description || "")}</p></div><span aria-hidden="true">↗</span></li>`).join("")}</ol>`;
    case "assurance":
      return `<aside class="assurance-note"><span class="overline">The current boundary</span><p>Kernel-checked evidence. Independently checked execution and erasure. Tests connecting the implementation to its specification.</p><p class="caption">The compiler as a whole is not yet formally verified.</p></aside>`;
    case "pipeline":
      return `<figure class="pipeline"><div class="pipeline-source">Source program</div><div class="pipeline-branch"><div><b>Checking path</b><span>Typed tree → checking IR</span><span>IR checker + proof kernel</span><em>Reference interpreter</em></div><div><b>Execution path</b><span>Typed tree → erased tree</span><span>Layout checks + Rust emission</span><em>Erased interpreter + compiled Rust</em></div></div><figcaption>The tests compare these execution paths; the proof kernel checks the evidence used by the program.</figcaption></figure>`;
    case "roadmap":
      return roadmapIndex(doc);
    case "performance":
      return performance(doc);
    default:
      throw Error(`Unknown component ${name}`);
  }
}
function roadmapIndex(doc) {
  const ordered = [...db.projects].sort(
    (a, b) =>
      (({ active: 0, planned: 1, done: 2 })[a.status] ?? 1) -
        ({ active: 0, planned: 1, done: 2 }[b.status] ?? 1) ||
      a.order - b.order,
  );
  return `<div class="roadmap-controls"><label class="filter-search">Find a project or task<input id="roadmap-search" type="search" placeholder="Search, or jump to LOC-232" autocomplete="off"></label><div class="filters" aria-label="Project status"><button data-filter="all" aria-pressed="true">All projects</button><button data-filter="active" aria-pressed="false">Active</button><button data-filter="planned" aria-pressed="false">Planned</button><button data-filter="done" aria-pressed="false">Completed</button></div><p id="roadmap-results" class="caption" role="status"></p></div><div class="project-list">${ordered
    .map((p) => {
      const ts = db.tasks.filter((t) => t.project === p.id);
      const done = ts.filter((t) => t.status === "done").length;
      const closed = ts.filter((t) =>
        ["done", "canceled"].includes(t.status),
      ).length;
      const search = [
        p.name,
        p.summary,
        ...ts.map((t) => `LOC-${t.n} ${t.title}`),
      ].join(" ");
      return `<article class="project" data-status="${esc(p.status)}" data-search="${esc(search.toLowerCase())}"><div class="project-meta"><span class="status ${esc(p.status)}">${esc(p.status === "done" ? "completed" : p.status)}</span><span>${done} / ${ts.length} tasks done</span></div><h2><a href="${relative(doc.route, p.route)}">${esc(p.name)}</a></h2><p>${esc(p.summary || "Open the project for its scope, tasks, and acceptance notes.")}</p><div class="project-progress" role="img" aria-label="${closed} of ${ts.length} tasks closed"><span style="width:${ts.length ? (100 * closed) / ts.length : 0}%"></span></div><a class="project-open" href="${relative(doc.route, p.route)}">Read the plan <span aria-hidden="true">→</span></a></article>`;
    })
    .join(
      "",
    )}</div><p class="filter-empty" hidden>No projects match. Try a task number or another keyword.</p>`;
}
const median = (values) => {
  values = [...values].sort((a, b) => a - b);
  const mid = Math.floor(values.length / 2);
  return values.length
    ? values.length % 2
      ? values[mid]
      : (values[mid - 1] + values[mid]) / 2
    : 0;
};
function performance(doc) {
  const { summary, runs } = db.performance;
  if (!runs.length)
    return '<aside class="empty-state"><h2>No measurements published yet.</h2><p>Run the benchmark recorder and make its data branch available to the site build. There are no placeholder timings.</p></aside>';
  const latest = runs.at(-1);
  const names = summary.workloads.map((w) => w.name);
  const rows = names.map((name) => {
    const samples = latest.samples.filter((s) => s.workload === name);
    return {
      name,
      search:
        median(
          samples.filter((s) => s.pass === "search").map((s) => s.total_ns),
        ) / 1e6,
      replay:
        median(
          samples
            .filter((s) => s.pass === "replay" || s.pass === "locked")
            .map((s) => s.total_ns),
        ) / 1e6,
      count: samples.filter((s) => s.pass === "search").length,
    };
  });
  const max = Math.max(...rows.map((r) => r.search), 1);
  return `<aside class="measurement-note"><span class="overline">Recorded benchmark · ${esc(latest.recorded_at.slice(0, 10))}</span><p>Source <a href="${db.repository}/commit/${latest.source_commit}"><code>${latest.source_commit.slice(0, 7)}</code></a>${latest.dirty ? " · working tree contained changes" : ""}. Historical measurements, not timings for the current source.</p></aside><div class="metric-strip"><div><strong>${runs.length}</strong><span>recorded runs</span></div><div><strong>${names.length}</strong><span>workloads in this epoch</span></div><div><strong>${summary.headline_index?.toFixed(1) || "—"}</strong><span>index · baseline 100</span></div></div><figure class="benchmark-figure"><figcaption><b>Compilation by workload</b><span>Median · milliseconds · lower is faster</span></figcaption><div class="chart-key"><span>Search</span><span>Locked replay</span></div>${rows.map((r) => `<div class="benchmark-row"><span class="workload" title="${esc(r.name)}">${esc(path.basename(r.name, ".lc"))}</span><div class="bars"><div style="width:${Math.max(0.7, (100 * r.search) / max)}%" title="Search: ${r.search.toFixed(2)} ms"></div><div style="width:${Math.max(0.7, (100 * r.replay) / max)}%" title="Replay: ${r.replay.toFixed(2)} ms"></div></div><span class="timing">${r.search.toFixed(2)} <small>/ ${r.replay.toFixed(2)}</small></span></div>`).join("")}<p class="caption">${esc(latest.pins.machine_class.system)} · ${esc(latest.pins.machine_class.architecture)} · ${esc(latest.pins.machine_class.processor)}. Search / replay medians are shown for the latest recorded run.</p></figure><details class="data-table"><summary>Inspect samples and run history</summary><table><thead><tr><th>Workload</th><th>Search (ms)</th><th>Replay (ms)</th><th>Search samples</th></tr></thead><tbody>${rows.map((r) => `<tr><td>${esc(path.basename(r.name))}</td><td>${r.search.toFixed(3)}</td><td>${r.replay.toFixed(3)}</td><td>${r.count}</td></tr>`).join("")}</tbody></table><ul>${runs.map((r) => `<li>${esc(r.recorded_at)} · <a href="${db.repository}/commit/${r.source_commit}">${r.source_commit.slice(0, 7)}</a>${r.dirty ? " · dirty checkout" : ""}</li>`).join("")}</ul><a href="${relative(doc.route, "data/performance.json")}">Download the displayed data (JSON)</a></details><p class="caption">Observed span: ${summary.observed_days.toFixed(2)} days. ${summary.two_week_window_complete ? "The two-week observation window is complete." : "The two-week observation window is still open."}</p>`;
}
function section(doc) {
  if (doc.spec_chapter === 1 || doc.id === "specification")
    return "specification";
  if (doc.kind === "project") return "roadmap";
  if (
    ["architecture", "kernel-contract", "formal-core", "correctness"].includes(
      doc.id,
    )
  )
    return "correctness";
  if (["performance", "generated-status"].includes(doc.id))
    return "performance";
  if (["examples", "readme", "development-workflow"].includes(doc.id))
    return "examples";
  return "home";
}
function sidebar(doc, toc) {
  if (doc.id === "home") return "";
  const active = section(doc);
  const ch =
    active === "specification"
      ? `<div class="sidebar-label">Language manual</div><ol class="book-nav">${chapters.map((d, i) => `<li><a ${d.id === doc.id ? 'aria-current="page"' : ""} href="${relative(doc.route, d.route)}"><span>${String(i + 1).padStart(2, "0")}</span>${esc(d.title)}</a></li>`).join("")}</ol>`
      : "";
  const outline = toc
    .filter((t) => t.depth === 2)
    .map((t) => `<li><a href="#${t.id}">${esc(t.title)}</a></li>`)
    .join("");
  return `<aside class="sidebar"><details class="chapter-menu" open><summary>${active === "specification" ? "In this book" : "On this page"}</summary>${ch}${outline ? `<div class="page-outline"><div class="sidebar-label">${ch ? "In this chapter" : "Contents"}</div><ol>${outline}</ol></div>` : ""}<div class="sidebar-bottom"><a href="${relative(doc.route, "reference/kernel.html")}">Kernel contract ↗</a><a href="${relative(doc.route, "reference/formal-core.html")}">Formal core ↗</a></div></details></aside>`;
}
function shell(doc, rendered) {
  const active = section(doc);
  const link = (target) => relative(doc.route, target);
  const at = chapters.indexOf(doc);
  const order = chapters.findIndex((d) => d.id === doc.id);
  const desc =
    doc.description ||
    plain(
      doc.body.find((l) => l && !l.startsWith("#") && !l.startsWith("<!--")) ||
        "Locus, a Rust-like language with kernel-checked proofs.",
    );
  const nav = NAV.map(
    ([id, title, route]) =>
      `<a href="${link(route)}" ${active === id ? 'aria-current="page"' : ""}>${title}</a>`,
  ).join("");
  const pager =
    order >= 0
      ? `<nav class="chapter-pager" aria-label="Chapter navigation">${order > 0 ? `<a href="${link(chapters[order - 1].route)}"><small>← Previous</small>${esc(chapters[order - 1].title)}</a>` : "<span></span>"}${order < chapters.length - 1 ? `<a href="${link(chapters[order + 1].route)}"><small>Next →</small>${esc(chapters[order + 1].title)}</a>` : ""}</nav>`
      : "";
  const title =
    doc.id === "home"
      ? "Locus — systems code with checked proofs"
      : `${doc.title} · Locus`;
  const src = doc.source
    ? `<a href="${sourceURL(doc.source)}">Edit this page ↗</a><a href="${link("source/" + doc.source)}">Read the Markdown</a>`
    : "";
  return `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light"><meta name="description" content="${esc(desc)}"><title>${esc(title)}</title><link rel="icon" href="${link("assets/mark.svg")}" type="image/svg+xml"><link rel="stylesheet" href="${link("assets/site.css")}"><script src="${link("assets/search-index.js")}" defer></script><script src="${link("assets/site.js")}" defer></script></head><body class="${doc.id === "home" ? "home" : "reading"}" data-root="${link("index.html").replace(/index\.html$/, "")}"><a class="skip-link" href="#main">Skip to content</a><div class="masthead-note"><span>A language experiment</span><span>Code, claims, and checked evidence</span></div><header class="masthead"><a class="wordmark" href="${link("index.html")}" aria-label="Locus home"><span class="locus-mark" aria-hidden="true">⌖</span> locus<span class="wordmark-period">.</span></a><nav aria-label="Main navigation">${nav}</nav><div class="header-tools"><button class="open-search" type="button" aria-label="Search the Locus documentation"><span>Search</span><kbd>⌘ K</kbd></button><a class="github-link" href="${db.repository}">GitHub ↗</a></div></header><div class="page-frame">${sidebar(doc, rendered.toc)}<main id="main"><div class="chapter-kicker">${doc.id === "home" ? "Locus / A programming language" : doc.kind === "project" ? "Roadmap / Project" : doc.spec_chapter === 1 ? `Language manual / ${String(order + 1).padStart(2, "0")}` : active === "correctness" ? "Evidence / Implementation" : active === "performance" ? "Observations / Measurements" : active === "roadmap" ? "Work in the open" : "Locus / " + esc(doc.title)}</div><article class="prose ${doc.id === "home" ? "home-prose" : ""}">${rendered.html}</article>${pager}<div class="page-source">${src}</div></main></div><footer class="footer"><a class="footer-name" href="${link("index.html")}">locus.</a><p>Programs with explicit guarantees.<br>A small core, built in the open.</p><div><a href="${link("correctness.html")}">How we check our work</a><a href="${db.repository}">Source on GitHub ↗</a><span>${db.dirty ? "Working tree preview" : "Source"} · ${db.revision.slice(0, 7)}</span></div></footer><dialog id="search-dialog" aria-labelledby="search-title"><div class="search-top"><label id="search-title" for="site-search">Search the manual and roadmap</label><button id="close-search" aria-label="Close search" type="button">Esc</button></div><input id="site-search" type="search" placeholder="Try erasure, borrow, 1.5:4, or LOC-232" autocomplete="off"><p id="search-status" role="status">Search chapters, rules, and tasks.</p><ol id="search-results"></ol></dialog></body></html>\n`;
}
function projectPage(p) {
  const doc = { ...p, title: p.name, body: [], kind: "project" };
  const ts = db.tasks
    .filter((t) => t.project === p.id)
    .sort((a, b) => a.n - b.n);
  const intro = markdown(`# ${p.name}\n\n${p.description || ""}`, doc).html;
  const html =
    intro +
    `<div class="project-summary"><span class="status ${p.status}">${esc(p.status)}</span><span>${ts.filter((t) => t.status === "done").length} completed · ${ts.filter((t) => !["done", "canceled"].includes(t.status)).length} open · ${ts.filter((t) => t.status === "canceled").length} superseded</span></div><div class="task-list">${ts
      .map((t) => {
        searches.push({
          title: `LOC-${t.n} · ${t.title}`,
          text: plain(t.notes),
          url: taskTarget(t.n),
          kind: "Task",
        });
        const body = markdown(t.notes || "No additional notes.", {
          ...doc,
          route: p.route,
          anchorPrefix: `task-${t.n}-`,
        }).html;
        return `<details class="task" id="LOC-${t.n}"><summary><a class="task-number" href="#LOC-${t.n}">LOC-${t.n}</a><span>${esc(t.title)}</span><small class="status ${esc(t.status)}">${esc(t.status === "canceled" ? "superseded" : t.status === "doing" ? "in progress" : t.status)}</small></summary><div class="task-body prose">${body}</div></details>`;
      })
      .join("")}</div>`;
  return [doc, { html, toc: [] }];
}
for (const d of docs) {
  const rendered = markdown(d.body.join("\n"), d);
  searches.push({
    title: d.title,
    text: plain(d.description || d.body.join(" ")).slice(0, 6000),
    url: d.route,
    kind: d.spec_chapter ? "Chapter" : "Page",
  });
  outputFiles.set(d.route, shell(d, rendered));
  outputFiles.set(
    "source/" + d.source,
    fs.readFileSync(path.join(ROOT, d.source)),
  );
}
for (const p of db.projects) {
  const [d, rendered] = projectPage(p);
  outputFiles.set(p.route, shell(d, rendered));
  outputFiles.set(
    "source/" + p.source,
    fs.readFileSync(path.join(ROOT, p.source)),
  );
}
outputFiles.set(
  "assets/search-index.js",
  "window.LOCUS_SEARCH=" +
    JSON.stringify(searches).replace(/</g, "\\u003c") +
    ";\n",
);
outputFiles.set(
  "data/performance.json",
  JSON.stringify(db.performance, null, 2) + "\n",
);
outputFiles.set(
  "data/spec-index.json",
  JSON.stringify(
    {
      coverage: db.coverage,
      rules: db.rules.map((r) => ({
        ...r,
        url: ruleTarget(r.id),
        tests: (ruleUses.get(r.id) || []).map(({ excerpt, ...c }) => c),
      })),
    },
    null,
    2,
  ) + "\n",
);
outputFiles.set(".nojekyll", "");
outputFiles.set(".locus-site", "Generated by website/build.mjs\n");
const legacyDocs = Object.fromEntries(docs.map((d) => [d.id, d.route]));
legacyDocs.language = "specification/introduction.html";
const legacyTasks = Object.fromEntries(
  db.tasks.map((t) => [
    t.id,
    db.tasks.filter((other) => other.id === t.id).length === 1
      ? taskTarget(t.n)
      : "roadmap.html",
  ]),
);
const legacyRules = Object.fromEntries(
  db.rules.map((r) => [r.id, ruleTarget(r.id)]),
);
outputFiles.set(
  "atlas.html",
  `<!doctype html><html lang="en"><meta charset="utf-8"><title>Locus documentation</title><p>The Atlas is now the <a href="index.html">Locus website</a>.</p><script>const docs=${JSON.stringify(legacyDocs)}, tasks=${JSON.stringify(legacyTasks)}, rules=${JSON.stringify(legacyRules)};const parts=decodeURIComponent(location.hash).replace(/^#\\//,'').split('/');let target='index.html';if(parts[0]==='doc'){target=parts[2]?.startsWith('spec-')?rules[parts[2].slice(5)]:docs[parts[1]];if(parts[2]&&!parts[2].startsWith('spec-')&&target)target+='#'+parts[2].replace(/^h-/,'');}if(parts[0]==='task')target=tasks[parts[1]]||'roadmap.html';if(['roadmap','project','tasks'].includes(parts[0]))target='roadmap.html';location.replace(target||'index.html');</script></html>`,
);
for (const name of ["site.css", "site.js", "mark.svg"])
  outputFiles.set(
    "assets/" + name,
    fs.readFileSync(path.join(ROOT, "website/assets", name)),
  );
// Stage complete output: a failed render leaves the previous preview intact.
fs.mkdirSync(path.dirname(output), { recursive: true });
const stage = fs.mkdtempSync(path.join(path.dirname(output), ".locus-site-"));
for (const [name, value] of outputFiles) {
  const target = path.join(stage, name);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, value);
}
const checked = spawnSync(
  python,
  [path.join(ROOT, "tools/site.py"), "validate", "--out", stage],
  { cwd: ROOT, encoding: "utf8" },
);
if (checked.status !== 0) {
  fs.rmSync(stage, { recursive: true, force: true });
  process.stderr.write(checked.stdout + checked.stderr);
  process.exit(1);
}
fs.rmSync(output, { recursive: true, force: true });
fs.renameSync(stage, output);
console.log(
  `Built ${docs.length + db.projects.length} Markdown pages · ${db.rules.length} rules · ${db.tasks.length} tasks → ${path.relative(ROOT, output)}`,
);
process.stdout.write(checked.stdout);
