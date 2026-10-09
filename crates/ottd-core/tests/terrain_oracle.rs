//! Native geometry oracle input contract and checked geometry boundaries.
use ottd_core::terrain::{Corner, Edge, Foundation, Slope, TilePixel, slope_from_corners};
use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn pixel_rounding_and_halftile_seams_match_native_conventions() -> Result {
    // Given a western rise and its discontinuous western foundation.
    let west = Slope::new(1)?;
    let half = Slope::new(33)?;
    // When sampling across the diagonal seam, native rounding is asymmetric.
    let pixel = |v| TilePixel::new(v);
    assert_eq!(west.partial_pixel_z(pixel(1)?, pixel(0)?), 0);
    assert_eq!(half.partial_pixel_z(pixel(1)?, pixel(0)?), 8);
    assert_eq!(half.partial_pixel_z(pixel(0)?, pixel(0)?), 0);
    assert_eq!(half.edge_pixel_z(Edge::NorthWest), (8, 8));
    Ok(())
}

#[test]
fn malformed_geometry_is_rejected_at_the_boundary() -> Result {
    // Given bytes outside the native defined domains.
    for raw in [16, 17, 21, 31, 64, 128, 255] {
        assert!(Slope::new(raw).is_err());
    }
    assert!(Corner::try_from(4).is_err());
    assert!(Edge::try_from(4).is_err());
    assert!(Foundation::try_from(14).is_err());
    assert!(TilePixel::new(16).is_err());
    assert!(Slope::new(33)?.corner_z(Corner::West).is_err());
    assert!(
        Slope::new(0)?
            .apply_foundation(Foundation::InclinedX)
            .is_err()
    );
    assert!(slope_from_corners([0, 2, 0, 0]).is_err());
    assert!(slope_from_corners([0, 3, 1, 1]).is_err());
    Ok(())
}

#[test]
fn steep_corner_heights_preserve_orientation() -> Result {
    // Given N,W,E,S corners around a steep western rise.
    let (slope, base) = slope_from_corners([8, 9, 7, 8])?;
    // Then native bit encoding and foundation height agree.
    assert_eq!((slope.raw(), base), (27, 7));
    assert_eq!(slope.corner_z(Corner::West)?, 2);
    let (top, height) = slope.apply_foundation(Foundation::SteepBoth)?;
    assert_eq!((top.raw(), height), (33, 1));
    Ok(())
}

#[test]
#[ignore = "requires fresh native vectors; scripts/check-terrain.sh"]
fn terrain_matches_native_vectors() -> Result {
    let path = std::env::var_os("OTTD_TERRAIN_JSON").ok_or("OTTD_TERRAIN_JSON missing")?;
    let vectors: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let slopes = vectors
        .get("slopes")
        .and_then(Value::as_array)
        .ok_or("slopes")?;
    assert_eq!(slopes.len(), 100);
    let mut seen_slopes = std::collections::BTreeSet::new();
    for row in slopes {
        let raw = u8::try_from(row["slope"].as_u64().ok_or("slope")?)?;
        assert!(seen_slopes.insert(raw), "duplicate slope {raw}");
        let slope = Slope::new(raw)?;
        let pixels = row["pixels"].as_array().ok_or("pixels")?;
        assert_eq!(pixels.len(), 256);
        for (offset, expected) in pixels.iter().enumerate() {
            let x = TilePixel::new(u8::try_from(offset % 16)?)?;
            let y = TilePixel::new(u8::try_from(offset / 16)?)?;
            assert_eq!(
                u64::from(slope.partial_pixel_z(x, y)),
                expected.as_u64().ok_or("height")?,
                "slope={raw} pixel={offset}"
            );
        }
        for edge in 0..4 {
            let (near, far) = slope.edge_pixel_z(Edge::try_from(edge)?);
            assert_eq!(
                serde_json::json!([near, far]),
                *row.get("edges")
                    .and_then(|v| v.get(usize::from(edge)))
                    .ok_or("edge")?
            );
        }
        if raw & 32 == 0 {
            for corner in 0..4 {
                assert_eq!(
                    u64::from(slope.corner_z(Corner::try_from(corner)?)?),
                    row.get("corners")
                        .and_then(|v| v.get(usize::from(corner)))
                        .and_then(Value::as_u64)
                        .ok_or("corner")?
                );
            }
        }
    }
    let foundations = vectors
        .get("foundations")
        .and_then(Value::as_array)
        .ok_or("foundations")?;
    assert_eq!(foundations.len(), 232);
    let mut seen_foundations = std::collections::BTreeSet::new();
    for row in foundations {
        let slope = Slope::new(u8::try_from(row["slope"].as_u64().ok_or("slope")?)?)?;
        let foundation = Foundation::try_from(u8::try_from(
            row["foundation"].as_u64().ok_or("foundation")?,
        )?)?;
        assert!(seen_foundations.insert((slope.raw(), foundation as u8)));
        let (top, dz) = slope.apply_foundation(foundation)?;
        assert_eq!(serde_json::json!([top.raw(), dz]), row["result"]);
    }
    Ok(())
}
