#!/usr/bin/env python3
"""Atlas rule IDs, focused test citations, and executable Now fences.

IDs are explicit, permanent chapter.section:paragraph names. Never regenerate
existing IDs when prose moves. A specification paragraph is one Markdown block
(prose paragraph, contiguous list, table, or fence); headings organize blocks.
Categories are local: omitting one means informative, never inherited.

  python3 tools/spec.py check
  python3 tools/spec.py inventory
  python3 tools/spec.py fences [--out DIRECTORY]

Focused citations are //~ spec: ID[, ID] in a corpus file under 40 physical lines,
or #[doc = "spec: ID[, ID]"] on one #[test] Rust function. A citation must point
to a real paragraph; an asserting test must cite an operative rule, not only
informative/example prose. Broad files may cite rules for context but do not
satisfy focused coverage. Review still judges whether a test exercises its rule.
"""
from __future__ import annotations
import argparse
from dataclasses import asdict, dataclass
import json
from pathlib import Path
import re
import sys
import content

ROOT = Path(__file__).resolve().parent.parent
ATLAS_BLOCK = re.compile(r'<script type="application/json" id="atlas-data">\s*(.*?)\s*</script>', re.S)
ID = r'\d+\.\d+:\d+'
ANNOTATION = re.compile(r'<!--\s*spec:\s*(' + ID + r')(?:\s+([a-z-]+))?\s*-->$')
FENCE = re.compile(r'^\s{0,3}(`{3,}|~{3,})(.*)$')
CATEGORIES = {'normative', 'legality-rule', 'syntax', 'dynamic-semantics', 'informative', 'example'}
OPERATIVE = CATEGORIES - {'informative', 'example'}
SPEC_DOCS = {'language': 1, 'kernel-contract': 2, 'formal-core': 3}

@dataclass
class Block:
    start: int
    end: int
    kind: str
    text: str

@dataclass
class Paragraph:
    id: str
    category: str
    doc: str
    line: int
    text: str

@dataclass
class Citation:
    source: str
    test: str
    ids: list[str]
    focused: bool

@dataclass
class Fence:
    doc: str
    line: int
    language: str
    mode: str
    code: str
    expected: list[str]
    reason: str = ''
    visible_code: str = ''
    complete_code: str = ''
    excerpt: bool = False

def load(path: Path = ROOT/'docs') -> dict:
    if path.is_dir():
        return content.load(path.parent if path.name == 'docs' else path)
    # Read-only legacy fixture support. The production specification is Markdown.
    match = ATLAS_BLOCK.search(path.read_text())
    if not match:
        raise ValueError(f'{path}: missing atlas-data block')
    return json.loads(match.group(1))

def blocks(lines: list[str]):
    """One consistent Markdown-block definition for annotation and checking."""
    i = 0
    while i < len(lines):
        if not lines[i].strip() or re.match(r'^#{1,6}\s', lines[i]):
            i += 1
            continue
        start = i
        if ANNOTATION.fullmatch(lines[i].strip()):
            i += 1
            yield Block(start, i, 'annotation', lines[start])
            continue
        opened = FENCE.match(lines[i])
        if opened:
            close = re.compile(r'^\s{0,3}' + re.escape(opened[1][0]) + '{' + str(len(opened[1])) + r',}\s*$')
            i += 1
            while i < len(lines) and not close.fullmatch(lines[i]):
                i += 1
            if i == len(lines):
                raise ValueError(f'line {start+1}: unclosed code fence')
            i += 1
            yield Block(start, i, 'fence', '\n'.join(lines[start:i]))
            continue
        while i < len(lines) and lines[i].strip():
            if i > start and (FENCE.match(lines[i]) or ANNOTATION.fullmatch(lines[i].strip()) or re.match(r'^#{1,6}\s', lines[i])):
                break
            i += 1
        yield Block(start, i, 'text', '\n'.join(lines[start:i]))

def inventory(data: dict) -> list[Paragraph]:
    result = []
    seen = set()
    for doc in data['docs']:
        if doc.get('spec_chapter', SPEC_DOCS.get(doc['id'])) is None:
            continue
        pending = None
        for block in blocks(doc['body']):
            if block.kind == 'annotation':
                if pending:
                    raise ValueError(f"{doc['id']}:{block.start+1}: annotation without a paragraph")
                pending = ANNOTATION.fullmatch(block.text.strip())
                continue
            if pending is None:
                raise ValueError(f"{doc['id']}:{block.start+1}: paragraph has no stable spec ID: {block.text[:70]}")
            identity, category = pending[1], pending[2] or 'informative'
            pending = None
            if identity in seen:
                raise ValueError(f'duplicate spec ID {identity}')
            if not identity.startswith(str(doc.get('spec_chapter', SPEC_DOCS.get(doc['id']))) + '.'):
                raise ValueError(f"{identity}: wrong chapter for {doc['id']}")
            if category not in CATEGORIES:
                raise ValueError(f'{identity}: unknown category {category}')
            seen.add(identity)
            result.append(Paragraph(identity, category, doc['id'], block.start+1, block.text))
        if pending:
            raise ValueError(f"{doc['id']}: trailing annotation without a paragraph")
    return result

def parse_ids(text: str, source: str) -> list[str]:
    words = re.split(r'[\s,]+', text.strip())
    if not words or any(not re.fullmatch(ID, word) for word in words):
        raise ValueError(f'{source}: invalid spec citation {text!r}')
    return words

def citations(root: Path) -> list[Citation]:
    result = []
    for directory in ['tests/corpus', 'examples']:
        for path in sorted((root / directory).rglob('*.lc')):
            text = path.read_text()
            identities = []
            for match in re.finditer(r'//~\s*spec:\s*([^\n]+)', text):
                identities.extend(parse_ids(match[1], str(path)))
            if identities:
                # Directives/comments do not turn a large test into a small one.
                lines = len(text.splitlines())
                result.append(Citation(str(path.relative_to(root)), path.stem, identities, lines < 40))
    attr = re.compile(r'(?m)^[ \t]*#\[doc\s*=\s*"spec:\s*([^"\n]+)"\]\s*$')
    test = re.compile(r'(?m)^#\[test\]\s*\n((?:\s*#\[[^\n]+\]\s*\n|\s*//[^\n]*\n)*)\s*(?:pub\s+)?fn\s+(\w+)\s*\(')
    for path in sorted((root / 'tests').rglob('*.rs')):
        text = path.read_text()
        matched = set()
        for function in test.finditer(text):
            identities = []
            for annotation in attr.finditer(function[1]):
                identities.extend(parse_ids(annotation[1], str(path)))
                matched.add(function.start(1) + annotation.start())
            if identities:
                result.append(Citation(str(path.relative_to(root)), function[2], identities, True))
        for annotation in attr.finditer(text):
            if annotation.start() not in matched:
                raise ValueError(f'{path}: spec attribute must be attached after #[test] on one function')
    return result

def validate(paragraphs: list[Paragraph], uses: list[Citation]) -> dict:
    by_id = {p.id: p for p in paragraphs}
    covered = set()
    failures = []
    for citation in uses:
        missing = set(citation.ids) - by_id.keys()
        if missing:
            failures.append(f'{citation.source}::{citation.test}: dangling citation(s): {", ".join(sorted(missing))}')
            continue
        operative = [identity for identity in citation.ids if by_id[identity].category in OPERATIVE]
        if not operative:
            failures.append(f'{citation.source}::{citation.test}: asserting test cites only informative/example paragraphs')
        if citation.focused:
            covered.update(operative)
    for paragraph in paragraphs:
        if paragraph.category in OPERATIVE and paragraph.id not in covered:
            failures.append(f'{paragraph.id} ({paragraph.doc}:{paragraph.line}): operative paragraph has no focused test')
    if failures:
        raise ValueError('\n'.join(failures))
    return {'paragraphs': len(paragraphs), 'normative': sum(p.category in OPERATIVE for p in paragraphs), 'citations': sum(len(c.ids) for c in uses), 'focused_tests': sum(c.focused for c in uses)}

def example_display(code: str, location: str) -> tuple[str, str, bool]:
    """Select teaching lines without changing the source given to the compiler.

    Markers are reserved whole-line comments, balanced within one code fence.
    Keep both views: a short excerpt and the complete copyable program.
    """
    visible, complete = [], []
    hidden = False
    excerpt = False
    for number, line in enumerate(code.splitlines(), 1):
        marker = line.strip()
        if marker.startswith('// docs:'):
            if marker not in {'// docs:hide', '// docs:show'}:
                raise ValueError(f'{location}, code line {number}: unknown docs marker {marker!r}')
            if marker == '// docs:hide':
                if hidden:
                    raise ValueError(f'{location}, code line {number}: nested docs:hide')
                hidden = excerpt = True
            else:
                if not hidden:
                    raise ValueError(f'{location}, code line {number}: docs:show without docs:hide')
                hidden = False
            continue
        complete.append(line)
        if not hidden:
            visible.append(line)
    if hidden:
        raise ValueError(f'{location}: unclosed docs:hide; add docs:show before the fence ends')
    if excerpt and not any(line.strip() for line in visible):
        raise ValueError(f'{location}: excerpt must contain visible code')
    return '\n'.join(visible).strip('\n'), '\n'.join(complete).strip('\n'), excerpt

def fences(data: dict) -> list[Fence]:
    result = []
    for doc in data['docs']:
        if doc.get('group') != 'Now':
            continue
        for block in blocks(doc['body']):
            if block.kind != 'fence':
                continue
            lines = block.text.splitlines()
            info = FENCE.match(lines[0])[2].strip().split()
            if len(info) < 2 or info[1] not in {'check', 'run', 'reject', 'prose'}:
                raise ValueError(f"{doc['id']}:{block.start+1}: every Now fence needs LANGUAGE check/run/reject/prose")
            language, mode = info[:2]
            rest = info[2:]
            if mode != 'prose' and language not in {'rust', 'locus', 'lc'}:
                raise ValueError(f"{doc['id']}:{block.start+1}: executable Locus fence must use rust (legacy locus and lc are also accepted)")
            if mode == 'reject' and (not rest or any(not re.fullmatch(r'L\d{4}', code) for code in rest)):
                raise ValueError(f"{doc['id']}:{block.start+1}: reject fence needs diagnostic codes")
            if mode == 'prose' and not rest:
                raise ValueError(f"{doc['id']}:{block.start+1}: prose fence needs a reason (e.g. kernel-grammar or shell-commands)")
            code = '\n'.join(lines[1:-1]) + '\n'
            if mode == 'run' and not re.search(r'//~\s*run:', code):
                raise ValueError(f"{doc['id']}:{block.start+1}: run fence has no expected run value")
            visible, complete, excerpt = example_display(code, f"{doc['id']}:{block.start+1}")
            if excerpt and mode == 'prose':
                raise ValueError(f"{doc['id']}:{block.start+1}: docs markers require a checked Locus example")
            result.append(Fence(doc['id'], block.start+1, language, mode, code, rest if mode == 'reject' else [], ' '.join(rest) if mode == 'prose' else '', visible, complete, excerpt))
    return result

def known_markers(root: Path) -> list[tuple[str, str, str]]:
    result = []
    for directory in ['tests/corpus', 'examples']:
        for path in sorted((root / directory).rglob('*.lc')):
            for match in re.finditer(r'//~\s*known:\s*(LOC-\d+)\s+([^\n]+)', path.read_text()):
                result.append((str(path.relative_to(root)), match[1], match[2]))
    for path in sorted((root/'tests').rglob('*.rs')):
        for match in re.finditer(r'known_bug\(\s*"(LOC-\d+)"\s*,\s*"([^"\n]+)"', path.read_text()):
            result.append((str(path.relative_to(root)), match[1], match[2]))
    return result

def validate_known(data: dict, markers: list[tuple[str, str, str]]) -> int:
    tasks = {'LOC-' + str(task['n']): task for task in data['tasks']}
    for source, name, reason in markers:
        if not reason.strip():
            raise ValueError(f'{source}: {name} known-bug marker has no reason')
        if name not in tasks:
            raise ValueError(f'{source}: {name} known-bug task does not exist')
        if tasks[name]['status'] in {'done', 'canceled'}:
            raise ValueError(f'{source}: {name} known-bug task is closed; remove or update the marker')
    return len(markers)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['check', 'inventory', 'fences'])
    parser.add_argument('--docs', '--atlas', dest='atlas', type=Path, default=ROOT/'docs')
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args(argv)
    try:
        data = load(args.atlas)
        if args.command == 'check':
            result = validate(inventory(data), citations(args.root))
            result['now_fences'] = len(fences(data))
            result['known_bugs'] = validate_known(data, known_markers(args.root))
            print('spec traceability: ' + ', '.join(f'{key}={value}' for key, value in result.items()))
        elif args.command == 'inventory':
            print(json.dumps([asdict(p) for p in inventory(data)], indent=2))
        else:
            exported = [asdict(f) for f in fences(data)]
            if args.out:
                args.out.mkdir(parents=True, exist_ok=True)
                for index, fence in enumerate(exported):
                    name = f"{fence['doc']}-{index}.lc"
                    (args.out/name).write_text(fence['code'])
                    fence['file'] = name
                (args.out/'manifest.json').write_text(json.dumps(exported, indent=2)+'\n')
                (args.out/'manifest.tsv').write_text(''.join(
                    '\t'.join([f['doc'], str(f['line']), f['mode'], f['file'], ','.join(f['expected'])]) + '\n'
                    for f in exported))
            else:
                print(json.dumps(exported, indent=2))
    except (ValueError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0

if __name__ == '__main__':
    sys.exit(main())
