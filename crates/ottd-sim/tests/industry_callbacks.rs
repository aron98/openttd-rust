//! Original-economy industry monthly boundary.
use ottd_sim::{IndustryCallbacks, IndustryMonth, run_industry_month};

fn empty() -> Result<IndustryCallbacks, serde_json::Error> {
    serde_json::from_str(
        r#"{"economy_type":0,"newgrf":false,"industry_density":4,
        "map_width":64,"map_height":64,"current_company":14,"random_state":[1,2],
        "wanted_inds":0,"industries":[]}"#,
    )
}
#[test]
fn builder_increases_fraction_on_small_map() -> Result<(), Box<dyn std::error::Error>> {
    // Given an empty 64x64 original economy with automatic industry building.
    let before = empty()?;
    // When the real monthly boundary runs.
    let after = run_industry_month(
        before,
        IndustryMonth {
            month: 0,
            year: 2001,
            days_since_last_month: 31,
        },
    )?;
    // Then ceiling-scaled monthly growth is119.4375 rounded up to120.
    assert_eq!(after.wanted_inds, 120);
    assert_eq!((after.current_company, after.random_state), (14, [1, 2]));
    Ok(())
}
#[test]
fn smooth_economy_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given an economy requiring unported random production and closure.
    let mut before = empty()?;
    before.economy_type = 1;
    // When the monthly callback is requested.
    let result = run_industry_month(
        before,
        IndustryMonth {
            month: 0,
            year: 2001,
            days_since_last_month: 31,
        },
    );
    // Then it cannot silently return partial statistics.
    assert!(result.is_err());
    Ok(())
}

fn industry() -> ottd_sim::CallbackIndustry {
    ottd_sim::CallbackIndustry {
        id: 0,
        industry_type: 0,
        prod_level: 16,
        last_prod_year: 2000,
        valid_history: 0,
        produced: vec![ottd_sim::IndustryProduced {
            cargo: 254,
            waiting: 17,
            rate: 2,
            history: vec![
                ottd_sim::ProducedHistory {
                    production: 1,
                    transported: 0
                };
                61
            ],
        }],
        accepted: vec![],
    }
}
#[test]
fn closure_marked_industry_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given a pending closure whose deletion requires the world map and stations.
    let mut before = empty()?;
    let mut entry = industry();
    entry.prod_level = 0;
    before.industries.push(entry);
    // When monthly processing is requested.
    let result = run_industry_month(
        before,
        IndustryMonth {
            month: 0,
            year: 2001,
            days_since_last_month: 31,
        },
    );
    // Then partial statistics without the native deletion branch are forbidden.
    assert!(result.is_err());
    Ok(())
}
#[test]
fn incomplete_history_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given a truncated history array.
    let mut before = empty()?;
    let mut entry = industry();
    entry
        .produced
        .first_mut()
        .ok_or("missing slot")?
        .history
        .pop();
    before.industries.push(entry);
    // When monthly processing is requested.
    let result = run_industry_month(
        before,
        IndustryMonth {
            month: 0,
            year: 2001,
            days_since_last_month: 31,
        },
    );
    // Then the boundary rejects missing records.
    assert!(result.is_err());
    Ok(())
}
#[test]
fn high_cargo_byte_remains_valid_in_native_callback() -> Result<(), Box<dyn std::error::Error>> {
    // Given cargo254, which native IsValidCargoType accepts because it is not255.
    let mut before = empty()?;
    before.industries.push(industry());
    // When monthly statistics roll over at the maximum pre-rewind year.
    let after = run_industry_month(
        before,
        IndustryMonth {
            month: 0,
            year: 5_000_001,
            days_since_last_month: 31,
        },
    )?;
    // Then the production year reflects that phase, not a later rewound clock.
    assert_eq!(
        after
            .industries
            .first()
            .ok_or("missing industry")?
            .last_prod_year,
        5_000_001
    );
    Ok(())
}
