#!/usr/bin/env python3
from __future__ import annotations

import unittest

from repo_truth_test_evidence_cases import EvidenceArtifactCases
from repo_truth_test_inventory_cases import InventoryAndBindingCases
from repo_truth_test_security_cases import SecurityAndManifestCases


class RepositoryTruthNegativeTests(
    SecurityAndManifestCases,
    EvidenceArtifactCases,
    InventoryAndBindingCases,
    unittest.TestCase,
):
    """Repository-truth negative cases grouped by validation responsibility."""


if __name__ == "__main__":
    unittest.main()
