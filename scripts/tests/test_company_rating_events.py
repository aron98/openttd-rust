"""Complete month ordering rejects missing and reordered passive observations."""

import unittest

from scripts.company_rating_events import Event, ordered
from scripts.company_rating_model import snapshot
from scripts.tests import company_rating_fixture as fixture


def events() -> tuple[Event, ...]:
    state = snapshot(fixture.snapshot())
    return tuple(
        Event(index, phase, 255, state)
        for index, phase in enumerate(
            (
                "month_enter",
                "statistics_enter",
                "bankruptcy_complete",
                "maintenance_complete",
                "interest_before",
                "interest_after",
                "month_leave",
            )
        )
    )


class EventTests(unittest.TestCase):
    def test_accepts_complete_month_when_no_companies(self) -> None:
        given = events()
        ordered(given)

    def test_rejects_incomplete_month_when_final_event_missing(self) -> None:
        given = events()[:-1]
        with fixture.rejects():
            ordered(given)

    def test_rejects_early_interest_when_maintenance_not_observed(self) -> None:
        given = list(events())
        given[3], given[4] = given[4], given[3]
        with fixture.rejects():
            ordered(tuple(given))

    def test_rejects_duplicate_sequence_when_phase_order_correct(self) -> None:
        given = list(events())
        given[1] = Event(0, given[1].phase, 255, given[1].snapshot)
        with fixture.rejects():
            ordered(tuple(given))

    def test_rejects_unknown_owner_when_global_phase(self) -> None:
        given = list(events())
        given[0] = Event(0, given[0].phase, 0, given[0].snapshot)
        with fixture.rejects():
            ordered(tuple(given))

    def test_rejects_missing_rating_when_quarter_has_company(self) -> None:
        state = snapshot(fixture.owned())
        given = tuple(
            Event(row.sequence, row.phase, row.owner, state) for row in events()
        )
        with fixture.rejects():
            ordered(given)

    def test_accepts_owner_sequence_when_quarter_complete(self) -> None:
        state = snapshot(fixture.owned())
        phases = [(row.phase, row.owner) for row in events()]
        phases[4:4] = [
            (name, 0) for name in ("history_before", "history_after", "rating_complete")
        ]
        given = tuple(
            Event(index, phase, owner, state)
            for index, (phase, owner) in enumerate(phases)
        )
        ordered(given)
