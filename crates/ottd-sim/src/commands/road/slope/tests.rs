use super::{Input, InputError, check};

#[test]
fn host_domain_rejects_nonterrain_slopes_and_each_invalid_mask() {
    for slope in 0..=u8::MAX {
        assert_eq!(
            Input::new(slope, 0, 0, 0, false, 1).is_ok(),
            matches!(slope, 0..=14 | 23 | 27 | 29 | 30)
        );
    }
    for bits in 16..=u8::MAX {
        assert_eq!(
            Input::new(0, bits, 0, 0, false, 1),
            Err(InputError::RequestedBits)
        );
        assert_eq!(
            Input::new(0, 0, bits, 0, false, 1),
            Err(InputError::ExistingBits)
        );
        assert_eq!(
            Input::new(0, 0, 0, bits, false, 1),
            Err(InputError::OtherBits)
        );
    }
}

#[test]
fn failed_slope_keeps_autocompleted_requested_bits() -> Result<(), InputError> {
    let result = check(Input::new(5, 1, 0, 0, false, 187)?);
    assert_eq!(result.pieces, 5);
    assert!(!result.cost.success);
    assert_eq!(result.cost.error.as_deref(), Some("CMD_ERROR"));
    assert_eq!((result.cost.cost, result.cost.expenses), (0, 255));
    Ok(())
}

#[test]
fn no_new_bits_fails_before_flat_success() -> Result<(), InputError> {
    let result = check(Input::new(0, 3, 3, 0, true, 187)?);
    assert_eq!(result.pieces, 0);
    assert!(!result.cost.success);
    let added = check(Input::new(0, 3, 1, 0, true, 187)?);
    assert_eq!(added.pieces, 2);
    assert_eq!((added.cost.cost, added.cost.expenses), (0, 255));
    assert!(added.cost.success);
    Ok(())
}

#[test]
fn zero_foundation_cost_retains_construction_expense() -> Result<(), InputError> {
    for price in [0, 1, -1, i64::MIN, i64::MAX] {
        let result = check(Input::new(1, 1, 0, 0, true, price)?);
        assert_eq!((result.cost.cost, result.cost.expenses), (price, 0));
        assert_eq!(result.pieces, 1);
        assert!(result.cost.success);
    }
    let other = check(Input::new(1, 1, 0, 1, true, 187)?);
    assert_eq!((other.cost.cost, other.cost.expenses), (0, 255));
    Ok(())
}

#[test]
fn helper_foundation_result_preserves_caller_setting_gate() -> Result<(), InputError> {
    let result = check(Input::new(3, 2, 8, 0, false, 77)?);
    assert!(result.cost.success);
    assert_eq!(result.pieces, 10);
    assert_eq!((result.cost.cost, result.cost.expenses), (77, 0));
    Ok(())
}

#[test]
fn steep_shapes_reduce_to_the_actual_highest_corner() -> Result<(), InputError> {
    for (steep, corner) in [(23, 2), (27, 1), (29, 8), (30, 4)] {
        for bits in 0..16 {
            assert_eq!(
                check(Input::new(steep, bits, 0, 0, true, 187)?),
                check(Input::new(corner, bits, 0, 0, true, 187)?)
            );
        }
    }
    Ok(())
}
