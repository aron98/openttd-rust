//! The complete pinned raw score vector and its bounded weighted total.
use super::{State, WorldTickError, company_rating_history, company_vehicle_rating, unsupported};
use crate::runtime::stations::saved::StationRow;

const SCORE_INFO: [(i64, i64); 9] = [
    (120, 100),
    (80, 100),
    (10_000, 100),
    (50_000, 50),
    (100_000, 100),
    (40_000, 400),
    (8, 50),
    (10_000_000, 50),
    (250_000, 50),
];
fn total(parts: &[i64; 10]) -> Result<i64, WorldTickError> {
    parts
        .iter()
        .zip(SCORE_INFO)
        .try_fold(0_i64, |sum, (part, (needed, weight))| {
            let contribution = (*part)
                .clamp(0, needed)
                .saturating_mul(weight)
                .checked_div(needed)
                .ok_or_else(|| unsupported("company_quarter", "score divisor"))?;
            Ok(sum.saturating_add(contribution))
        })
}
pub(super) fn parts(
    state: &State<'_>,
    owner: u32,
    facilities: i64,
) -> Result<[i64; 10], WorldTickError> {
    let (vehicles, minimum) = company_vehicle_rating::vehicles(state, owner)?;
    let companies = state.table(b"PLYR")?;
    let company = StationRow {
        schema: companies.schema(),
        record: companies
            .record(owner)
            .ok_or_else(|| unsupported("company_quarter", "company"))?,
    };
    let (schema, rows) = company.rows("old_economy")?;
    let [min_income, max_income, delivered, variety] =
        company_rating_history::history(schema, rows)?;
    let mut parts = [
        vehicles,
        facilities,
        minimum,
        min_income,
        max_income,
        delivered,
        variety,
        state.number(b"PLYR", owner, "money")?.max(0),
        250_000_i64.saturating_sub(state.number(b"PLYR", owner, "current_loan")?),
        0,
    ];
    let score = total(&parts)?;
    let [.., slot] = &mut parts;
    *slot = score;
    // The pinned weights sum to SCORE_MAX=1000, so native return scaling is identity.
    Ok(parts)
}
#[cfg(test)]
mod tests {
    use super::total;
    #[test]
    fn raw_components_clamp_and_truncate_independently() {
        assert_eq!(
            total(&[1, 2, 3256, 0, 431, 799, 1, 90_981, 150_000, 999]).unwrap(),
            77
        );
        assert_eq!(
            total(&[
                120, 80, 10_000, 50_000, 100_000, 40_000, 8, 10_000_000, 250_000, -1
            ])
            .unwrap(),
            1000
        );
        assert_eq!(total(&[i64::MIN; 10]).unwrap(), 0);
        assert_eq!(total(&[i64::MAX; 10]).unwrap(), 1000);
    }
}
