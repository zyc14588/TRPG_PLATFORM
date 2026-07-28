from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest.mock import patch

from manifest import manifest_source_errors, render
from release_readiness import (
    REQUIRED_RELEASE_TEST_CASES,
    assess,
    readiness_report_errors,
    release_evidence_errors,
    release_junit_errors,
)
from repo_truth import (
    ROOT,
    canonical_json_sha256,
    compose_services,
    false_skip_markers,
    git_modes,
    sha256_file,
    stable_service_version_output,
    validate_evidence,
)
from validate_workflows import validate as validate_workflows
from verify_evidence_schema import schema_errors
from verify_manifest import HASHED_ROW, manifest_count_errors
from verify_test_inventory import (
    _blank_rust_non_code,
    integration_test_silent_env_successes,
    inventory,
)
from verify_compose_security import errors as compose_security_errors

# Mixin modules import the complete shared test vocabulary explicitly.
__all__ = [name for name in globals() if not name.startswith("__")]
