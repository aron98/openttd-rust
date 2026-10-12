//! Synthetic arithmetic and ownership controls, not native full-pool coverage.
use super::{CompanyProfits, GroupProfitCaches, NamedProfit, ProfitFields};
use std::collections::BTreeMap;

fn cache() -> GroupProfitCaches {
    GroupProfitCaches {
        companies: BTreeMap::from([
            (0, CompanyProfits::default()),
            (1, CompanyProfits::default()),
        ]),
        named: BTreeMap::from([
            (
                0,
                NamedProfit {
                    owner: 0,
                    vehicle_type: 1,
                    parent: 65535,
                    profit: ProfitFields::default(),
                },
            ),
            (
                1,
                NamedProfit {
                    owner: 0,
                    vehicle_type: 1,
                    parent: 0,
                    profit: ProfitFields::default(),
                },
            ),
            (
                2,
                NamedProfit {
                    owner: 1,
                    vehicle_type: 1,
                    parent: 65535,
                    profit: ProfitFields::default(),
                },
            ),
        ]),
    }
}

#[test]
fn named_contribution_does_not_roll_up_or_cross_owner() {
    let mut cache = cache();
    cache.add_road(0, 1, -1122, true).unwrap();
    assert_eq!(cache.named.get(&1).unwrap().profit.profit_last_year, -1122);
    assert_eq!(cache.named.get(&0).unwrap().profit, ProfitFields::default());
    assert_eq!(cache.named.get(&2).unwrap().profit, ProfitFields::default());
    assert_eq!(cache.companies.get(&1).unwrap(), &CompanyProfits::default());
    assert_eq!(
        cache.companies.get(&0).unwrap().all[1].profit_last_year_min_age,
        -1122
    );
    assert_eq!(
        cache.companies.get(&0).unwrap().default_group[1],
        ProfitFields::default()
    );
}

#[test]
fn foreign_group_rejection_preserves_owned_cache() {
    let mut cache = cache();
    let before = cache.clone();
    assert!(cache.add_road(0, 2, 99, true).is_err());
    assert_eq!(cache, before);
}

#[test]
fn cloned_cache_has_no_shared_mutable_owner() {
    let original = cache();
    let mut candidate = original.clone();
    candidate.add_road(0, 65534, 1601, false).unwrap();
    assert_eq!(original, cache());
    assert_eq!(
        candidate.companies.get(&0).unwrap().all[1].profit_last_year,
        1601
    );
    assert_eq!(
        candidate.companies.get(&0).unwrap().all[1].num_vehicle_min_age,
        0
    );
}

#[test]
fn native_money_saturates_and_minimum_count_wraps() {
    let mut profit = ProfitFields {
        profit_last_year: i64::MAX - 1,
        profit_last_year_min_age: i64::MIN + 1,
        num_vehicle_min_age: u16::MAX,
    };
    profit.add(7, false);
    assert_eq!(profit.profit_last_year, i64::MAX);
    profit.add(-7, true);
    assert_eq!(profit.profit_last_year_min_age, i64::MIN);
    assert_eq!(profit.num_vehicle_min_age, 0);
}

#[test]
fn reaching_minimum_age_preserves_all_age_sum() {
    let mut profit = ProfitFields {
        profit_last_year: 41,
        profit_last_year_min_age: -7,
        num_vehicle_min_age: u16::MAX,
    };
    profit.reached_min_age(19);
    assert_eq!(profit.profit_last_year, 41);
    assert_eq!(profit.profit_last_year_min_age, 12);
    assert_eq!(profit.num_vehicle_min_age, 0);
}
