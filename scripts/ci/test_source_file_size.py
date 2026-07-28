#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path, PurePosixPath

from check_source_file_size import (
    MAX_SOURCE_LINES,
    SourceSizeViolation,
    is_human_maintained_source,
    physical_line_count,
    source_size_violations,
)


class SourceFileSizeContractTests(unittest.TestCase):
    def test_source_suffixes_include_tests_but_exclude_non_source_areas(self) -> None:
        self.assertTrue(is_human_maintained_source(PurePosixPath("src/service.rs")))
        self.assertTrue(
            is_human_maintained_source(PurePosixPath("tests/integration.py"))
        )
        self.assertFalse(
            is_human_maintained_source(PurePosixPath("docs/example.rs"))
        )
        self.assertFalse(
            is_human_maintained_source(PurePosixPath("tests/fixtures/sample.py"))
        )
        self.assertFalse(
            is_human_maintained_source(PurePosixPath("migrations/001.sql"))
        )
        self.assertFalse(
            is_human_maintained_source(PurePosixPath("src/architecture.md"))
        )

    def test_physical_lines_include_a_final_unterminated_line(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.py"
            path.write_bytes(b"first\nsecond")
            self.assertEqual(physical_line_count(path), 2)
            path.write_bytes(b"first\nsecond\n")
            self.assertEqual(physical_line_count(path), 2)

    def test_only_files_above_the_limit_are_reported(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            within_limit = PurePosixPath("within.py")
            over_limit = PurePosixPath("over.rs")
            ignored = PurePosixPath("docs/oversized.js")
            (root / within_limit).write_text(
                "pass\n" * MAX_SOURCE_LINES,
                encoding="utf-8",
            )
            (root / over_limit).write_text(
                "// line\n" * (MAX_SOURCE_LINES + 1),
                encoding="utf-8",
            )
            (root / ignored.parent).mkdir()
            (root / ignored).write_text(
                "// line\n" * (MAX_SOURCE_LINES + 10),
                encoding="utf-8",
            )

            self.assertEqual(
                source_size_violations(
                    root,
                    [within_limit, over_limit, ignored],
                ),
                [SourceSizeViolation(over_limit, MAX_SOURCE_LINES + 1)],
            )


if __name__ == "__main__":
    unittest.main()
