#!/usr/bin/env python3
"""Validate the standalone SpectraLang language guide.

The validator intentionally uses only Python's standard library.  It checks the
offline HTML contract, extracts runnable SpectraLang blocks into temporary
files, and validates the repository fixtures named by the guide.
"""

from __future__ import annotations

import argparse
import html.parser
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any


GUIDE_RELATIVE = Path("docs/spectralang-language-guide.html")
REQUIRED_IDS = {
    "top",
    "what-is-spectralang",
    "first-program",
    "modules",
    "core-syntax",
    "functions",
    "control-flow",
    "composite-types",
    "traits",
    "errors",
    "async",
    "stdlib",
    "ai-ml",
    "interop",
    "packages",
    "tooling",
    "maturity",
    "quick-reference",
    "api-scope",
}
REQUIRED_HEADINGS = (
    "What SpectraLang is and how to install it",
    "Your first program and the CLI workflow",
    "Source files, modules, imports, visibility, and project layout",
    "Variables, mutability, primitive types, literals, strings, and operators",
    "Functions, return types, implicit returns, closures, and function types",
    "Conditions, loops, ranges,",
    "Arrays, tuples, records, enums, pattern matching, and destructuring",
    "Traits,",
    "Option",
    "Async/await and structured concurrency",
    "Standard library reference",
    "Tensor programming, autodiff, and AI/ML",
    "Python, Rust, C ABI, and",
    "Packages, manifests, workspaces, lockfiles, registries, and offline workflows",
    "CLI, formatter, lint, diagnostics, LSP, VS Code, benchmarks, and testing",
    "Stable, beta, experimental, and deferred feature matrix",
    "Quick reference",
)
LEGACY_SYNTAX = re.compile(
    r"(^|\W)(?:fn|struct|pub|elif|elseif|unless)(?=\W|$)|=>|->|;",
    re.MULTILINE,
)


class GuideParser(html.parser.HTMLParser):
    """Collect the small HTML subset needed by the validation contract."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.hrefs: list[tuple[str, int]] = []
        self.resources: list[tuple[str, str, int]] = []
        self.headings: list[tuple[int, str, int]] = []
        self.meta: dict[str, str] = {}
        self.html_lang = ""
        self.title = ""
        self.code_blocks: list[dict[str, Any]] = []
        self.fixtures: list[dict[str, Any]] = []
        self._stack: list[str] = []
        self._heading: dict[str, Any] | None = None
        self._title_parts: list[str] = []
        self._in_title = False
        self._code_block: dict[str, Any] | None = None
        self._code_block_depth: int | None = None
        self._capturing_code = False
        self._style_text: list[str] = []
        self._script_text: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        line = self.getpos()[0]
        attributes = {key: value or "" for key, value in attrs}
        self._stack.append(tag)

        element_id = attributes.get("id")
        if element_id:
            self.ids.add(element_id)

        if tag == "html":
            self.html_lang = attributes.get("lang", "")

        if tag == "title":
            self._title_parts = []
            self._in_title = True

        if tag == "meta":
            key = attributes.get("name") or attributes.get("property")
            if key:
                self.meta[key.lower()] = attributes.get("content", "")

        if tag == "a" and attributes.get("href"):
            self.hrefs.append((attributes["href"], line))

        if tag in {"link", "script", "img"}:
            resource = attributes.get("href") if tag == "link" else attributes.get("src")
            if resource:
                self.resources.append((tag, resource, line))

        if tag in {"h1", "h2", "h3", "h4"}:
            self._heading = {"level": int(tag[1]), "parts": [], "line": line, "tag": tag}

        if tag == "div" and "data-spectra-code" in attributes:
            self._code_block = {
                "kind": attributes.get("data-kind", "snippet"),
                "source": attributes.get("data-source", ""),
                "parts": [],
                "line": line,
            }
            self._code_block_depth = len(self._stack)

        if tag == "code" and self._code_block is not None:
            self._capturing_code = True

        source = attributes.get("data-validate-source")
        if source:
            self.fixtures.append(
                {
                    "source": source,
                    "mode": attributes.get("data-validate-mode", "check"),
                    "line": line,
                }
            )

    def handle_endtag(self, tag: str) -> None:
        if self._heading is not None and tag == self._heading["tag"]:
            value = " ".join("".join(self._heading["parts"]).split())
            self.headings.append((self._heading["level"], value, self._heading["line"]))
            self._heading = None

        if tag == "title" and self._in_title:
            self.title = " ".join("".join(self._title_parts).split())
            self._in_title = False

        if tag == "code" and self._code_block is not None:
            self._capturing_code = False

        if (
            self._code_block is not None
            and tag == "div"
            and self._code_block_depth == len(self._stack)
        ):
            block = dict(self._code_block)
            block["text"] = "".join(block.pop("parts"))
            self.code_blocks.append(block)
            self._code_block = None
            self._code_block_depth = None
            self._capturing_code = False

        if self._stack:
            self._stack.pop()

    def handle_data(self, data: str) -> None:
        if self._heading is not None:
            self._heading["parts"].append(data)
        if self._in_title:
            self._title_parts.append(data)
        if self._code_block is not None and self._capturing_code:
            self._code_block["parts"].append(data)
        if self._stack and self._stack[-1] == "style":
            self._style_text.append(data)
        if self._stack and self._stack[-1] == "script":
            self._script_text.append(data)

    @property
    def style_text(self) -> str:
        return "".join(self._style_text)

    @property
    def script_text(self) -> str:
        return "".join(self._script_text)


def command_output(
    command: list[str],
    root: Path,
    timeout: int = 180,
) -> tuple[int, str]:
    """Run a validation command and return a compact combined output."""

    try:
        completed = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 1, str(exc)

    output = (completed.stdout or "") + (completed.stderr or "")
    if len(output) > 4000:
        output = output[-4000:]
    return completed.returncode, output.strip()


def safe_fixture(root: Path, relative: str) -> Path:
    """Resolve a repository fixture and reject paths outside the checkout."""

    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as exc:
        raise ValueError(f"fixture escapes repository root: {relative}") from exc
    return candidate


def report_command_failure(
    errors: list[str],
    label: str,
    line: int,
    command: list[str],
    output: str,
) -> None:
    rendered = " ".join(command)
    details = output or "the command returned no diagnostic output"
    errors.append(
        f"{label} (HTML line {line}) failed: {rendered}\n  {details.replace(chr(10), chr(10) + '  ')}"
    )


def validate(root: Path, binary: Path) -> list[str]:
    errors: list[str] = []
    guide = root / GUIDE_RELATIVE
    if not guide.is_file():
        return [f"guide not found: {guide}"]

    raw = guide.read_text(encoding="utf-8")
    parser = GuideParser()
    try:
        parser.feed(raw)
        parser.close()
    except Exception as exc:  # pragma: no cover - defensive parser report
        return [f"HTML parser failed: {exc}"]

    if parser.html_lang.lower() != "en":
        errors.append("HTML root must declare lang=\"en\"")

    if "SpectraLang Language Guide" not in parser.title:
        errors.append("title must identify SpectraLang Language Guide")
    if parser.meta.get("version") != "0.2.7" or "0.2.7" not in raw:
        errors.append("guide metadata must identify version 0.2.7")
    if parser.meta.get("language") != "SpectraLang":
        errors.append("guide metadata must identify language SpectraLang")
    if not parser.meta.get("description"):
        errors.append("guide metadata must include a description")
    if not parser.meta.get("pdf-conversion"):
        errors.append("guide metadata must describe PDF conversion")

    for required_id in sorted(REQUIRED_IDS):
        if required_id not in parser.ids:
            errors.append(f"required id is missing: #{required_id}")

    heading_text = "\n".join(value for _, value, _ in parser.headings)
    for required_heading in REQUIRED_HEADINGS:
        if required_heading not in heading_text:
            errors.append(f"required heading text is missing: {required_heading}")

    for href, line in parser.hrefs:
        if href.startswith("#") and href[1:] and href[1:] not in parser.ids:
            errors.append(f"broken table-of-contents/anchor link at HTML line {line}: {href}")

    for tag, resource, line in parser.resources:
        if resource.startswith(("http://", "https://")):
            errors.append(f"external runtime resource at HTML line {line}: <{tag} {resource}>")
        elif tag in {"link", "script"} and not resource.startswith("data:"):
            errors.append(f"non-inline runtime resource at HTML line {line}: <{tag} {resource}>")

    style = parser.style_text + raw
    if "@page" not in style:
        errors.append("print CSS must include @page")
    if not re.search(r"size\s*:\s*A4", style, re.IGNORECASE):
        errors.append("print CSS must declare A4 page size")
    if not re.search(r"@media\s+print", style, re.IGNORECASE):
        errors.append("print CSS must include @media print")
    for required_print_rule in ("sidebar", "toolbar", "copy-button", "break-inside", "break-before"):
        if required_print_rule not in style:
            errors.append(f"print CSS is missing the expected rule marker: {required_print_rule}")

    scope_text = ""
    api_scope_pattern = re.compile(
        r'<(?:div|section)[^>]+id=["\']api-scope["\'][^>]*>(.*?)</(?:div|section)>',
        re.IGNORECASE | re.DOTALL,
    )
    scope_match = api_scope_pattern.search(raw)
    if scope_match:
        scope_text = re.sub(r"<[^>]+>", " ", scope_match.group(1))
    if "spectra.api" not in scope_text or "outside" not in scope_text.lower():
        errors.append("the guide must explicitly identify spectra.api as outside its scope")

    for block_index, block in enumerate(parser.code_blocks, start=1):
        if block["kind"] != "runnable":
            continue
        label = block["source"] or f"runnable code block #{block_index}"
        code = block["text"].strip("\n")
        for match in LEGACY_SYNTAX.finditer(code):
            line_number = code[: match.start()].count("\n") + 1
            source_line = code.splitlines()[line_number - 1].strip()
            errors.append(
                f"{label} (HTML line {block['line']}): legacy syntax at extracted line "
                f"{line_number}: {source_line}"
            )
        if not code.strip():
            errors.append(f"{label} (HTML line {block['line']}) is empty")
            continue
        if not binary.is_file():
            errors.append(f"SpectraLang binary not found for {label}: {binary}")
            continue
        with tempfile.TemporaryDirectory(prefix="spectralang-guide-") as temp_dir:
            source = Path(temp_dir) / "guide_block.spectra"
            source.write_text(code + "\n", encoding="utf-8")
            command = [str(binary), "check", str(source)]
            return_code, output = command_output(command, root)
            if return_code != 0:
                report_command_failure(errors, label, block["line"], command, output)

    for fixture in parser.fixtures:
        label = fixture["source"]
        try:
            source = safe_fixture(root, label)
        except ValueError as exc:
            errors.append(f"{label} (HTML line {fixture['line']}): {exc}")
            continue
        if not source.is_file():
            errors.append(f"{label} (HTML line {fixture['line']}): fixture not found")
            continue
        if not binary.is_file():
            errors.append(f"{label} (HTML line {fixture['line']}): SpectraLang binary not found: {binary}")
            continue
        mode = fixture["mode"]
        if mode not in {"check", "run"}:
            errors.append(
                f"{label} (HTML line {fixture['line']}): unsupported validation mode {mode!r}"
            )
            continue
        command = [str(binary), mode, str(source)]
        return_code, output = command_output(command, root)
        if return_code != 0:
            report_command_failure(errors, label, fixture["line"], command, output)

    if "data-spectra-code" not in raw:
        errors.append("guide must contain at least one data-spectra-code block")
    if "data-validate-source" not in raw:
        errors.append("guide must name at least one checked-in validation fixture")

    return errors


def main() -> int:
    argument_parser = argparse.ArgumentParser(description=__doc__)
    argument_parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="SpectraLang repository root (default: inferred from this script)",
    )
    argument_parser.add_argument(
        "--binary",
        type=Path,
        default=None,
        help="spectralang executable (default: target/debug/spectralang.exe)",
    )
    args = argument_parser.parse_args()

    root = args.root.resolve()
    binary = (args.binary or (root / "target" / "debug" / "spectralang.exe")).resolve()
    errors = validate(root, binary)
    if errors:
        print("SpectraLang language guide validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    guide = root / GUIDE_RELATIVE
    print(f"Language guide validation passed: {guide}")
    print("  HTML structure, anchors, metadata, offline resources, and A4 print CSS: OK")
    print("  Runnable SpectraLang blocks and designated repository fixtures: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
