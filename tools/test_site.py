#!/usr/bin/env python3
"""Regression checks for canonical Markdown and static site integrity."""
import importlib.util
import json
import re
import html as html_text
from pathlib import Path
import tempfile
import unittest
import copy
from contextlib import chdir
from unittest import mock
import atlas
import content
import spec

loader = importlib.util.spec_from_file_location(
    "locus_site", Path(__file__).with_name("site.py")
)
site = importlib.util.module_from_spec(loader)
loader.loader.exec_module(site)


class MarkdownTests(unittest.TestCase):
    def fixture(self, root):
        (root / "docs/data").mkdir(parents=True)
        (root / "docs/data/state.json").write_text('{"meta": {"taskPrefix": "LOC"}}')
        (root / "docs/guide.md").write_text(
            content.markdown(
                {
                    "id": "guide",
                    "title": "Guide",
                    "route": "guide.html",
                    "group": "Now",
                },
                "# Guide\n\nA rule.",
            )
        )
        (root / "docs/project.md").write_text(
            content.markdown(
                {
                    "id": "plan",
                    "name": "Plan",
                    "kind": "project",
                    "route": "roadmap/plan.html",
                },
                '# Plan\n\nA project.\n\n<a id="LOC-9"></a>\n## LOC-9 · The task\n'
                '<!-- task: {"id":"t9","status":"backlog"} -->\n\nUses `#`, quotes, and Unicode →.',
            )
        )

    def test_canonical_roundtrip_preserves_prose_metadata_and_task_notes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            first = content.load(root)
            content.save(first, root)
            self.assertEqual(first, content.load(root))
            self.assertEqual(
                first["tasks"][0]["notes"], "Uses `#`, quotes, and Unicode →."
            )
            self.assertEqual(first["docs"][0]["body"], ["# Guide", "", "A rule."])

    def test_nested_toml_metadata_survives_save(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            guide = root / "docs/guide.md"
            source = guide.read_text().replace(
                "\n+++\n",
                '\n[extra]\nlabel = "ok"\nvalues = [{n = 3}, {n = 4}]\n+++\n',
                1,
            )
            guide.write_text(source)
            original = content.load(root)
            content.save(original, root)
            saved = content.load(root)
            # Inline-table formatting moves the body; the semantic content is unchanged.
            original["docs"][0].pop("body_line")
            saved["docs"][0].pop("body_line")
            self.assertEqual(original, saved)

    def test_malformed_or_duplicate_task_is_not_silently_dropped(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            project = root / "docs/project.md"
            original = project.read_text()
            project.write_text(original + "\n## LOC-10 · Missing metadata\n")
            with self.assertRaisesRegex(ValueError, "malformed LOC"):
                content.load(root)
            project.write_text(
                original + '\n## LOC-9 · Duplicate\n<!-- task: {"id":"t10"} -->\n'
            )
            with self.assertRaisesRegex(ValueError, "duplicate task LOC-9"):
                content.load(root)

    def test_unsafe_and_duplicate_routes_are_rejected(self):
        for route in ("../escape.html", "/absolute.html", "guide.html"):
            with self.subTest(route=route), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root)
                (root / "docs/extra.md").write_text(
                    content.markdown({"id": "extra", "route": route}, "# Extra")
                )
                with self.assertRaisesRegex(ValueError, "duplicate or unsafe route"):
                    content.load(root)

    def test_archived_plans_use_stable_project_identity_after_renaming(self):
        data = content.load()
        for alias in ("Core build", "Reconciliation"):
            with self.subTest(alias=alias):
                summary = atlas.plan(copy.deepcopy(data), alias)
                self.assertIn("commits in", summary)


class StaticTests(unittest.TestCase):
    def page(self, root, name, body):
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            '<!doctype html><html lang="en"><title>Test</title><main>'
            + body
            + "</main></html>"
        )

    def test_cli_resolves_relative_output_once_from_callers_directory(self):
        with tempfile.TemporaryDirectory() as directory, chdir(directory):
            with mock.patch(
                "sys.argv", ["site.py", "check", "--out", "preview"]
            ), mock.patch.object(site, "build") as build:
                self.assertEqual(site.main(), 0)
                build.assert_called_once_with(Path(directory).resolve() / "preview")

    def test_relative_links_and_rule_fragments_work_below_a_prefix(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "locus"
            root.mkdir()
            self.page(
                root, "index.html", '<a href="manual/types.html#spec-1.2:3">rule</a>'
            )
            self.page(
                root,
                "manual/types.html",
                '<p id="spec-1.2:3">Rule</p><a href="../index.html">Home</a>',
            )
            (root / "data").mkdir()
            (root / "data/spec-index.json").write_text(
                json.dumps(
                    {
                        "rules": [
                            {
                                "id": "1.2:3",
                                "url": "manual/types.html#spec-1.2:3",
                                "category": "legality-rule",
                                "tests": [{"focused": True}],
                            }
                        ]
                    }
                )
            )
            self.assertIn("2 HTML pages", site.validate(root))

    def test_broken_fragment_duplicate_id_and_path_escape_fail(self):
        for body, reason in [
            ('<a href="#missing">link</a>', "missing fragment"),
            ('<p id="same"></p><p id="same"></p>', "duplicate IDs"),
            ('<a href="../secret">link</a>', "missing local target"),
            ('<a href="javascript:alert(1)">link</a>', "unsafe link"),
        ]:
            with self.subTest(
                reason=reason
            ), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.page(root, "index.html", body)
                with self.assertRaisesRegex(ValueError, reason):
                    site.validate(root)

    def test_rendered_site_keeps_all_rules_tasks_tests_and_checked_examples(self):
        # The integration gate builds once; standalone unit tests don't need npm.
        root = content.ROOT / "target/site"
        if not root.exists():
            self.skipTest("run tools/site.py build for renderer integration checks")
        index = json.loads((root / "data/spec-index.json").read_text())
        data = content.load()
        tasks = []
        for project in data["projects"]:
            parser = site.Page()
            parser.feed((root / project["route"]).read_text())
            tasks.extend(i for i in parser.ids if i.startswith("LOC-"))
        self.assertEqual(set(tasks), {f'LOC-{t["n"]}' for t in data["tasks"]})
        self.assertEqual(len(tasks), len(data["tasks"]))
        self.assertEqual(
            {r["id"] for r in index["rules"]}, {r.id for r in spec.inventory(data)}
        )
        for rule in index["rules"]:
            html = (root / rule["url"].split("#")[0]).read_text()
            self.assertIn('id="spec-' + rule["id"] + '"', html)
            for test in rule["tests"]:
                self.assertIn(test["test"], html)
        search = (root / "assets/search-index.js").read_text()
        entries = json.loads(search.removeprefix("window.LOCUS_SEARCH=").rstrip(";\n"))
        self.assertTrue(any("Vec<T>" in e["text"] for e in entries))
        models = (root / "specification/models.html").read_text()
        self.assertIn("Vec&lt;Erased&gt;", models)
        self.assertNotIn("<Erased>", models)
        examples = (root / "examples.html").read_text()
        self.assertIn("checked example", examples)
        self.assertNotIn("~~~rust", examples)
        self.assertIn('class="language-locus"', examples)
        self.assertIn("<figcaption><span>locus</span>", examples)
        self.assertIn('class="hljs-', examples)
        self.assertIn("no broken fragments", site.validate(root))

    def test_checked_excerpts_offer_exact_complete_source_without_javascript(self):
        root = content.ROOT / "target/site"
        if not root.exists():
            self.skipTest("run tools/site.py build for renderer integration checks")
        data = content.load()
        documents = {doc["id"]: doc for doc in data["docs"]}
        excerpts = [f for f in spec.fences(data) if f.excerpt]
        self.assertTrue(excerpts, "the manual should exercise excerpt rendering")
        for fence in excerpts:
            page = (root / documents[fence.doc]["route"]).read_text()
            visible = re.findall(r'<div class="example-excerpt">(.*?)<details class="complete-example">', page, re.S)
            complete = re.findall(r'<details class="complete-example">(.*?)</details>', page, re.S)
            def source(fragment):
                code = re.search(r'<pre><code[^>]*>(.*?)</code></pre>', fragment, re.S)
                self.assertIsNotNone(code)
                return html_text.unescape(re.sub(r'<[^>]*>', '', code[1]))
            self.assertIn(fence.visible_code, [source(f) for f in visible])
            self.assertIn(fence.complete_code, [source(f) for f in complete])
            self.assertIn('aria-label="Copy excerpt"', page)
            self.assertIn('aria-label="Copy complete example"', page)
            self.assertIn('<summary>Complete checked example</summary>', page)
            self.assertNotIn('// docs:', ''.join(visible + complete))


if __name__ == "__main__":
    unittest.main()
