//! Source-ordered Money and cargo contributions from the valid saved history list.
use super::{State, WorldTickError, unsupported};
use crate::world_access::row_field;
use ottd_save::{TableRecord, TableSchema, WireValue};

#[derive(Default)]
struct HistoryScores {
    income: Option<(i64, i64)>,
    delivered: i64,
    variety: i64,
}
struct Quarter {
    income: i64,
    expenses: i64,
    cargo: [u32; 64],
}
impl HistoryScores {
    fn record(&mut self, index: usize, quarter: &Quarter) {
        if index < 12 {
            let net = quarter.income.saturating_add(quarter.expenses);
            self.income = Some(
                self.income
                    .map_or((net, net), |(min, max)| (min.min(net), max.max(net))),
            );
        }
        if index < 4 {
            let amount = quarter.cargo.iter().fold(0_i64, |total, count| {
                total.saturating_add(i64::from(*count))
            });
            self.delivered = self.delivered.saturating_add(amount);
        }
        if index == 0 {
            self.variety = quarter
                .cargo
                .iter()
                .filter(|count| **count != 0)
                .fold(0_i64, |total, _| total.saturating_add(1));
        }
    }
    fn parts(&self) -> [i64; 4] {
        let (min, max) = self.income.unwrap_or((0, 0));
        [min.max(0), max, self.delivered, self.variety]
    }
}
pub(super) fn history(
    schema: &TableSchema,
    rows: &[TableRecord],
) -> Result<[i64; 4], WorldTickError> {
    if rows.len() > 24 {
        return Err(unsupported("company_quarter", "history count"));
    }
    let mut scores = HistoryScores::default();
    for (index, row) in rows.iter().take(12).enumerate() {
        let WireValue::Array(values) = row_field(schema, row, "delivered_cargo")? else {
            return Err(unsupported("company_quarter", "cargo history"));
        };
        let mut cargo = [0; 64];
        if values.len() != cargo.len() {
            return Err(unsupported("company_quarter", "cargo history width"));
        }
        for (target, value) in cargo.iter_mut().zip(values) {
            *target = u32::try_from(State::number_value(value)?)
                .map_err(|_| unsupported("company_quarter", "cargo history amount"))?;
        }
        scores.record(
            index,
            &Quarter {
                income: State::number_value(row_field(schema, row, "income")?)?,
                expenses: State::number_value(row_field(schema, row, "expenses")?)?,
                cargo,
            },
        );
    }
    Ok(scores.parts())
}

#[cfg(test)]
mod tests {
    use super::{HistoryScores, Quarter};
    #[test]
    fn negative_maximum_is_retained_and_addition_saturates_before_extrema() {
        let mut scores = HistoryScores::default();
        scores.record(
            0,
            &Quarter {
                income: -30,
                expenses: 10,
                cargo: [0; 64],
            },
        );
        scores.record(
            1,
            &Quarter {
                income: -40,
                expenses: 0,
                cargo: [0; 64],
            },
        );
        assert_eq!(scores.parts(), [0, -20, 0, 0]);
        scores.record(
            2,
            &Quarter {
                income: i64::MAX,
                expenses: 1,
                cargo: [0; 64],
            },
        );
        scores.record(
            3,
            &Quarter {
                income: i64::MIN,
                expenses: -1,
                cargo: [0; 64],
            },
        );
        assert_eq!(scores.parts(), [0, i64::MAX, 0, 0]);
    }
    #[test]
    fn native_history_windows_and_cargo_width_bound_accumulation() {
        for count in [0, 1, 4, 12, 24] {
            let mut scores = HistoryScores::default();
            for index in 0..count {
                scores.record(
                    index,
                    &Quarter {
                        income: 100,
                        expenses: -20,
                        cargo: [u32::MAX; 64],
                    },
                );
            }
            assert_eq!(
                scores.parts(),
                [
                    if count == 0 { 0 } else { 80 },
                    if count == 0 { 0 } else { 80 },
                    i64::try_from(count.min(4)).unwrap() * 64 * i64::from(u32::MAX),
                    if count == 0 { 0 } else { 64 }
                ]
            );
        }
        let mut scores = HistoryScores::default();
        scores.record(
            12,
            &Quarter {
                income: i64::MAX,
                expenses: 0,
                cargo: [u32::MAX; 64],
            },
        );
        assert_eq!(scores.parts(), [0; 4]);
    }
}
