# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m unittest scripts.tests.test_script_vm_policy_assertions
"""Typed standard-library expectation shared by policy rejection checks."""

from collections.abc import Iterator
from contextlib import contextmanager

from scripts.world_check_support import WorldCheckError


@contextmanager
def world_check_rejected() -> Iterator[None]:
    """Require the admission boundary to reject with its specific error category."""
    try:
        yield
    except WorldCheckError:
        return
    raise AssertionError("Expected WorldCheckError rejection")
