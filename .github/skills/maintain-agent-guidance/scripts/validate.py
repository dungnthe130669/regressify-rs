"""Validate the repository's discoverable agent guidance without running the app."""

import argparse
from pathlib import Path
import re
import tempfile
import unittest
from urllib.parse import unquote, urlsplit

import yaml


class UniqueKeyLoader(yaml.SafeLoader):
    def construct_mapping(self, node, deep=False):
        self.flatten_mapping(node)
        mapping = {}
        for key_node, value_node in node.value:
            key = self.construct_object(key_node, deep=deep)
            if not isinstance(key, str) or key in mapping:
                raise yaml.constructor.ConstructorError(
                    None, None, "duplicate or non-string metadata key", key_node.start_mark
                )
            mapping[key] = self.construct_object(value_node, deep=deep)
        return mapping


def frontmatter(source):
    match = re.match(r"\A---\r?\n(.*?)\r?\n---(?:\r?\n|\Z)", source, re.DOTALL)
    if not match:
        raise ValueError("missing YAML frontmatter")
    metadata = yaml.load(match.group(1), Loader=UniqueKeyLoader)
    if not isinstance(metadata, dict):
        raise ValueError("metadata must be a mapping")
    return metadata


def validate(root):
    root = root.resolve()
    errors = []
    guide = root / "AGENTS.md"
    skills_root = root / ".github/skills"
    skills = sorted(skills_root.glob("*/SKILL.md"))
    if not guide.is_file():
        errors.append("AGENTS.md is missing")
    if not skills:
        errors.append("no discoverable skills found")
    for entry in sorted(skills_root.iterdir()) if skills_root.is_dir() else []:
        if entry.is_file() and entry.suffix == ".md":
            errors.append(f"{entry.relative_to(root)}: legacy flat skill file")
        elif entry.is_dir() and not (entry / "SKILL.md").is_file():
            errors.append(f"{entry.relative_to(root)}: missing SKILL.md")

    documents = [guide, root / ".github/copilot-instructions.md", *skills]
    documents.extend(sorted((root / ".github/prompts").rglob("*.prompt.md")))
    for document in documents:
        if not document.is_file():
            continue
        label = document.relative_to(root).as_posix()
        source = document.read_text(encoding="utf-8")
        if document == guide and len(source.splitlines()) > 90:
            errors.append(f"{label}: exceeds 90 lines")
        if document in skills:
            if len(source.splitlines()) > 120:
                errors.append(f"{label}: exceeds 120 lines")
            try:
                metadata = frontmatter(source)
                name = metadata.get("name")
                if (
                    not isinstance(name, str)
                    or not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name)
                    or len(name) > 64
                    or name != document.parent.name
                ):
                    errors.append(f"{label}: name must match its lowercase directory")
                description = metadata.get("description")
                if (
                    not isinstance(description, str)
                    or not description.strip()
                    or len(description) > 1024
                ):
                    errors.append(f"{label}: description must contain 1-1024 characters")
                for flag in ("user-invocable", "disable-model-invocation"):
                    if flag in metadata and not isinstance(metadata[flag], bool):
                        errors.append(f"{label}: {flag} must be a YAML boolean")
                if metadata.get("disable-model-invocation") is True:
                    errors.append(f"{label}: automatic skill discovery must remain enabled")
            except (ValueError, yaml.YAMLError) as error:
                errors.append(f"{label}: invalid metadata ({error})")

        if re.search(r"(?:\.github/skills/|\.\./skills/)[\w-]+\.md\b", source):
            errors.append(f"{label}: stale flat-skill reference")
        for target in re.findall(r"\[[^\]\n]*\]\(([^\s)]+)\)", source):
            parsed = urlsplit(target)
            if parsed.scheme in ("https", "http", "mailto"):
                continue
            if parsed.scheme or parsed.netloc:
                errors.append(f"{label}: unsupported link {target}")
                continue
            if not parsed.path:
                continue
            resolved = (document.parent / unquote(parsed.path)).resolve()
            if not resolved.is_relative_to(root):
                errors.append(f"{label}: link escapes repository: {target}")
            elif not resolved.exists():
                errors.append(f"{label}: broken link {target}")
    return errors


class GuidanceTests(unittest.TestCase):
    SOURCE = "---\nname: sample\ndescription: 'Use when testing guidance.'\n---\n# Sample\n"

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "AGENTS.md").write_text("# Guide\n", encoding="utf-8")
        self.skill = self.root / ".github/skills/sample/SKILL.md"
        self.skill.parent.mkdir(parents=True)
        self.skill.write_text(self.SOURCE, encoding="utf-8")

    def test_valid_guidance_and_relative_links(self):
        self.skill.write_text(
            self.SOURCE + "[Guide](../../../AGENTS.md) [Web](https://example.test)\n",
            encoding="utf-8",
        )
        self.assertEqual(validate(self.root), [])

    def test_metadata_errors(self):
        for source in (
            "# No metadata\n",
            "---\nname: [\n---\n",
            self.SOURCE.replace("name: sample", "name: other"),
            self.SOURCE.replace("name: sample", "name: sample\nname: sample"),
            self.SOURCE.replace("'Use when testing guidance.'", "[]"),
            self.SOURCE.replace("'Use when testing guidance.'", "''"),
            self.SOURCE.replace("'Use when testing guidance.'", "'" + "x" * 1025 + "'"),
            self.SOURCE.replace("name: sample", "name: sample\nuser-invocable: 'false'"),
            self.SOURCE.replace("name: sample", "name: sample\ndisable-model-invocation: true"),
        ):
            with self.subTest(source=source[:60]):
                self.skill.write_text(source, encoding="utf-8")
                self.assertTrue(validate(self.root))

    def test_broken_escaped_and_unsupported_links(self):
        for target in ("missing.md", "../../../../outside.md", "file:///tmp/guide.md"):
            with self.subTest(target=target):
                self.skill.write_text(self.SOURCE + f"[Guide]({target})\n", encoding="utf-8")
                self.assertTrue(validate(self.root))

    def test_encoded_spaces_and_fragments(self):
        (self.root / "More Help.md").write_text("# Help\n", encoding="utf-8")
        self.skill.write_text(
            self.SOURCE + "[Help](../../../More%20Help.md#help) [Local](#sample)\n",
            encoding="utf-8",
        )
        self.assertEqual(validate(self.root), [])

    def test_missing_guidance_and_skills(self):
        (self.root / "AGENTS.md").unlink()
        self.skill.unlink()
        self.assertEqual(len(validate(self.root)), 3)

    def test_flat_skills_and_stale_references(self):
        (self.skill.parent.parent / "old.md").write_text("# Old\n", encoding="utf-8")
        (self.root / "AGENTS.md").write_text("Read `.github/skills/old.md`.\n", encoding="utf-8")
        self.assertEqual(len(validate(self.root)), 2)

    def test_size_limits(self):
        (self.root / "AGENTS.md").write_text("guide\n" * 91, encoding="utf-8")
        self.skill.write_text(self.SOURCE + "detail\n" * 121, encoding="utf-8")
        self.assertEqual(len(validate(self.root)), 2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[4])
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuidanceTests)
        return not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful()
    errors = validate(arguments.root)
    if errors:
        for error in errors:
            print(error)
        return 1
    count = len(list((arguments.root / ".github/skills").glob("*/SKILL.md")))
    print(f"Agent guidance valid: {count} discoverable skills; metadata and local links checked.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())