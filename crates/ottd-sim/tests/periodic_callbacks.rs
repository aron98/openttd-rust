//! Boundary tests for real bookkeeping callbacks.
use ottd_sim::{CompanyCallbacks, CompanyExpenses, run_company_year};

#[test]
fn company_year_rotates_three_expense_rows() -> Result<(), Box<dyn std::error::Error>> {
    // Given three distinct signed annual expense rows.
    let before = CompanyCallbacks {
        show_finances: false,
        random_state: [1, 2],
        companies: vec![CompanyExpenses {
            id: 14,
            yearly_expenses: [[i64::MIN; 13], [i64::MAX; 13], [7; 13]],
        }],
    };
    // When the original yearly bookkeeping boundary runs.
    let after = run_company_year(before)?;
    // Then rows rotate without arithmetic on the signed Money values.
    assert_eq!(
        after
            .companies
            .first()
            .ok_or("missing company")?
            .yearly_expenses,
        [[0; 13], [i64::MIN; 13], [i64::MAX; 13]]
    );
    Ok(())
}

#[test]
fn company_finance_ui_is_rejected() {
    // Given a context requiring unsupported financial UI/sound effects.
    let before = CompanyCallbacks {
        show_finances: true,
        random_state: [1, 2],
        companies: vec![],
    };
    // When the callback is requested.
    let result = run_company_year(before);
    // Then unsupported effects are rejected.
    assert!(result.is_err());
}

#[test]
fn partial_station_goods_are_rejected() {
    // Given an incomplete station cargo table.
    let before = ottd_sim::StationCallbacks {
        random_state: [1, 2],
        stations: vec![ottd_sim::StationStatus {
            id: 0,
            goods: vec![],
        }],
    };
    // When the monthly callback is requested.
    let result = ottd_sim::run_station_month(before);
    // Then missing native slots are rejected.
    assert!(result.is_err());
}

#[test]
fn incomplete_house_map_is_rejected() {
    // Given a map whose geometry promises more raw tiles than it contains.
    let before = ottd_sim::HouseCallbacks {
        map: ottd_sim::Map {
            width: 64,
            height: 64,
            tiles: vec![],
        },
        random_state: [1, 2],
    };
    // When the yearly house scan is requested.
    let result = ottd_sim::run_house_year(before);
    // Then missing tiles are rejected before aging.
    assert!(result.is_err());
}
