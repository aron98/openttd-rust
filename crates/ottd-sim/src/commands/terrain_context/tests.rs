use super::*;

#[test]
fn nested_scope_retains_temporary_rating_until_outer_exit() -> Result<(), CommandError> {
    let mut context = TerrainContext::default();
    context.test(|outer| {
        assert_eq!(outer.depth, 1);
        outer.ratings.insert(7, 465);
        outer.test(|inner| {
            assert_eq!(inner.depth, 2);
            assert_eq!(inner.ratings.get(&7), Some(&465));
            inner.ratings.insert(7, 430);
            Ok(())
        })?;
        assert_eq!(outer.depth, 1);
        assert_eq!(outer.ratings.get(&7), Some(&430));
        Ok(())
    })?;
    assert_eq!(context.depth, 0);
    assert_eq!(context.ratings.get(&7), Some(&430));
    Ok(())
}

#[test]
fn next_outer_scope_clears_prior_temporary_ratings() -> Result<(), CommandError> {
    let mut context = TerrainContext::default();
    context.ratings.insert(7, 430);
    context.test(|inner| {
        assert!(inner.ratings.is_empty());
        Ok(())
    })?;
    Ok(())
}

#[test]
fn error_unwinds_every_scope_and_next_command_starts_clean() -> Result<(), CommandError> {
    let mut context = TerrainContext::default();
    let result: Result<(), CommandError> = context.test(|outer| {
        outer.ratings.insert(7, 465);
        outer.test(|inner| {
            assert_eq!(inner.depth, 2);
            Err(CommandError::Unsupported("scope test sentinel"))
        })
    });
    assert!(matches!(
        result,
        Err(CommandError::Unsupported("scope test sentinel"))
    ));
    assert_eq!(context.depth, 0);
    context.test(|inner| {
        assert!(inner.ratings.is_empty());
        assert_eq!(inner.depth, 1);
        Ok(())
    })?;
    Ok(())
}
