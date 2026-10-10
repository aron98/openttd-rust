//! Native pool identity and independent unit-number allocation contracts.
use ottd_sim::runtime::pools::{PoolAllocator, PoolError, UnitNumberAllocator};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn sparse_restore_preserves_native_cursor_and_free_highwater() -> Result {
    let mut pool = PoolAllocator::restore(130, 8, [73, 0, 129])?;
    assert_eq!(
        (
            pool.snapshot().first_free,
            pool.snapshot().first_unused,
            pool.snapshot().slots
        ),
        (0, 130, 130)
    );
    assert_eq!(pool.allocate()?, 1);
    pool.free(129)?;
    assert_eq!(
        (
            pool.snapshot().first_free,
            pool.snapshot().first_unused,
            pool.snapshot().slots
        ),
        (2, 130, 130)
    );
    pool.free(0)?;
    assert_eq!(pool.allocate()?, 0);
    assert_eq!(pool.allocate()?, 2);
    Ok(())
}

#[test]
fn exhaustion_growth_and_reset_match_pool_limits() -> Result {
    let mut pool = PoolAllocator::new(3, 2)?;
    assert_eq!(pool.allocate()?, 0);
    assert_eq!(pool.snapshot().slots, 2);
    assert_eq!(pool.allocate()?, 1);
    assert_eq!(pool.allocate()?, 2);
    assert_eq!(pool.snapshot().slots, 3);
    let full = pool.clone();
    assert_eq!(pool.allocate(), Err(PoolError::Exhausted));
    assert_eq!(pool, full);
    pool.free(1)?;
    assert_eq!(pool.allocate()?, 1);
    pool.reset();
    assert_eq!(pool.snapshot().first_unused, 0);
    assert_eq!(pool.snapshot().slots, 0);
    assert_eq!(pool.allocate()?, 0);
    Ok(())
}

#[test]
fn rejected_candidate_does_not_consume_live_ids() -> Result {
    let mut live = PoolAllocator::restore(130, 8, [0, 73])?;
    let before = live.clone();
    let mut candidate = live.clone();
    assert_eq!(candidate.allocate()?, 1);
    let staged = candidate.clone();
    assert_eq!(candidate.insert(73), Err(PoolError::Occupied(73)));
    assert_eq!(candidate, staged);
    assert_eq!(candidate.insert(130), Err(PoolError::OutOfRange(130)));
    assert_eq!(candidate.free(129), Err(PoolError::Missing(129)));
    assert_eq!(live, before);
    assert_eq!(live.allocate()?, 1);
    assert_eq!(live.allocate()?, 2);
    assert!(PoolAllocator::restore(130, 8, [2, 2]).is_err());
    assert!(PoolAllocator::new(0, 1).is_err());
    assert!(PoolAllocator::new(3, 0).is_err());
    assert!(PoolAllocator::new(3, 3).is_err());
    Ok(())
}

#[test]
fn unit_numbers_are_independent_idempotent_and_reserved() -> Result {
    let mut company_road = UnitNumberAllocator::default();
    let mut company_train = UnitNumberAllocator::default();
    let other_company_road = UnitNumberAllocator::default();
    for id in [0, 65535, 1, 1, 64, 65] {
        assert_eq!(company_road.use_id(id), id);
    }
    assert_eq!(company_road.next_id(), 2);
    assert_eq!(company_train.next_id(), 1);
    assert_eq!(other_company_road.next_id(), 1);
    company_train.use_id(1);
    company_road.release_id(1)?;
    company_road.release_id(2)?;
    company_road.release_id(0)?;
    company_road.release_id(65535)?;
    assert_eq!(company_road.next_id(), 1);
    assert_eq!(company_train.next_id(), 2);
    assert!(company_road.release_id(1000).is_err());
    for id in 1..65535 {
        company_road.use_id(id);
    }
    assert_eq!(company_road.next_id(), 65535);
    company_road.release_id(65534)?;
    assert_eq!(company_road.next_id(), 65534);
    Ok(())
}
