//! Native owner/primary vehicle count and strict minimum-age profit contribution.
use super::{State, WorldTickError, unsupported};
use crate::{runtime::stations::saved::StationRow, world_access::row_field};

#[derive(Clone, Copy)]
struct VehicleProfit {
    owner: u32,
    kind: u8,
    primary: bool,
    age: u64,
    raw_profit: i64,
}
#[derive(Default)]
struct VehicleScores {
    count: u32,
    minimum: Option<i64>,
}
impl VehicleScores {
    fn consider(&mut self, owner: u32, vehicle: VehicleProfit) {
        if vehicle.owner != owner || vehicle.kind >= 4 || !vehicle.primary {
            return;
        }
        if vehicle.raw_profit > 0 {
            self.count = self.count.wrapping_add(1);
        }
        if vehicle.age > 730 {
            self.minimum = Some(
                self.minimum
                    .map_or(vehicle.raw_profit, |old| old.min(vehicle.raw_profit)),
            );
        }
    }
    fn parts(&self) -> (i64, i64) {
        (
            i64::from(self.count),
            (self.minimum.unwrap_or(0) >> 8).max(0),
        )
    }
}
pub(super) fn vehicles(state: &State<'_>, owner: u32) -> Result<(i64, i64), WorldTickError> {
    let mut scores = VehicleScores::default();
    let table = state.table(b"VEHS")?;
    for (_, record) in table.records() {
        let row = StationRow {
            schema: table.schema(),
            record,
        };
        if row.number("type")? != 1 {
            return Err(unsupported("company_quarter", "non-road vehicle"));
        }
        let common = row.child("roadveh")?.child("common")?;
        if common.number("subtype")? != 1 || common.number("next")? != 0 {
            return Err(unsupported(
                "company_quarter",
                "non-front or articulated road vehicle",
            ));
        }
        scores.consider(
            owner,
            VehicleProfit {
                owner: u32::try_from(common.number("owner")?)
                    .map_err(|_| unsupported("company_quarter", "vehicle owner"))?,
                kind: 1,
                primary: true,
                age: u64::try_from(State::number_value(row_field(
                    common.schema,
                    common.record,
                    "economy_age",
                )?)?)
                .map_err(|_| unsupported("company_quarter", "vehicle economy age"))?,
                raw_profit: State::number_value(row_field(
                    common.schema,
                    common.record,
                    "profit_last_year",
                )?)?,
            },
        );
    }
    Ok(scores.parts())
}

#[cfg(test)]
mod tests {
    use super::{VehicleProfit, VehicleScores};
    #[test]
    fn raw_positive_fraction_counts_without_positive_display_minimum() {
        for (raw, count, minimum) in [(0, 0, 0), (1, 1, 0), (255, 1, 0), (256, 1, 1), (-1, 0, 0)] {
            let mut scores = VehicleScores::default();
            scores.consider(
                0,
                VehicleProfit {
                    owner: 0,
                    kind: 1,
                    primary: true,
                    age: 731,
                    raw_profit: raw,
                },
            );
            assert_eq!(scores.parts(), (count, minimum));
        }
    }
    #[test]
    fn minimum_uses_first_eligible_raw_profit_and_strict_age() {
        let mut scores = VehicleScores::default();
        for (age, raw) in [(730, -512), (731, 1025), (800, 513)] {
            scores.consider(
                0,
                VehicleProfit {
                    owner: 0,
                    kind: 1,
                    primary: true,
                    age,
                    raw_profit: raw,
                },
            );
        }
        assert_eq!(scores.parts(), (2, 2));
        scores.consider(
            0,
            VehicleProfit {
                owner: 0,
                kind: 1,
                primary: true,
                age: 731,
                raw_profit: -1,
            },
        );
        assert_eq!(scores.parts(), (2, 0));
    }
    #[test]
    fn foreign_nonprimary_and_noncompany_types_do_not_contribute() {
        let mut scores = VehicleScores::default();
        for (owner, kind, primary) in [(1, 1, true), (0, 1, false), (0, 4, true), (0, 255, true)] {
            scores.consider(
                0,
                VehicleProfit {
                    owner,
                    kind,
                    primary,
                    age: 731,
                    raw_profit: 512,
                },
            );
        }
        assert_eq!(scores.parts(), (0, 0));
        for kind in 0..4 {
            scores.consider(
                0,
                VehicleProfit {
                    owner: 0,
                    kind,
                    primary: true,
                    age: 731,
                    raw_profit: 512,
                },
            );
        }
        assert_eq!(scores.parts(), (4, 2));
    }
}
