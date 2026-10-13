"""Admission expectations distinguish rejection, acceptance and unrelated failure."""

import unittest

from scripts.tests.script_vm_policy_assertions import world_check_rejected
from scripts.world_check_support import WorldCheckError


class PolicyExpectationTests(unittest.TestCase):
    def test_an_admitted_operation_fails_the_rejection_expectation(self) -> None:
        try:
            with world_check_rejected():
                pass
        except AssertionError:
            return
        raise AssertionError("An admitted operation satisfied a rejection check")

    def test_actual_world_check_rejection_satisfies_the_expectation(self) -> None:
        with world_check_rejected():
            raise WorldCheckError("Rejected source identity")

    def test_an_unrelated_failure_propagates(self) -> None:
        failure = LookupError("Unrelated lookup failure")
        caught_failure = None
        try:
            with world_check_rejected():
                raise failure
        except LookupError as caught:
            caught_failure = caught
        assert caught_failure is failure
