use super::super::{CommandError, terrain_read::MapSize};

enum Shape {
    Rectangle {
        span: u32,
        remaining: u32,
        rows: u32,
    },
    Diagonal {
        x: i64,
        y: i64,
        a: i64,
        b: i64,
        end_a: i64,
        end_b: i64,
    },
}
pub(super) struct Tiles {
    size: MapSize,
    current: Option<u32>,
    shape: Shape,
}
impl Tiles {
    pub(super) fn new(
        size: MapSize,
        end: u32,
        start: u32,
        diagonal: bool,
    ) -> Result<Self, CommandError> {
        let width =
            std::num::NonZeroU32::new(size.width()).ok_or(CommandError::Overflow("map width"))?;
        let (x, y) = (end % width, end / width);
        let (x2, y2) = (start % width, start / width);
        if diagonal {
            let a = i64::from(y2)
                .saturating_add(i64::from(x2))
                .saturating_sub(i64::from(y))
                .saturating_sub(i64::from(x));
            let b = i64::from(y2)
                .saturating_sub(i64::from(x2))
                .saturating_sub(i64::from(y))
                .saturating_add(i64::from(x));
            Ok(Self {
                size,
                current: Some(end),
                shape: Shape::Diagonal {
                    x: i64::from(x),
                    y: i64::from(y),
                    a: 0,
                    b: 0,
                    end_a: a.saturating_add(if a > 0 { 1 } else { -1 }),
                    end_b: b.saturating_add(if b > 0 { 1 } else { -1 }),
                },
            })
        } else {
            let span = x.abs_diff(x2).saturating_add(1);
            Ok(Self {
                size,
                current: Some(
                    y.min(y2)
                        .saturating_mul(size.width())
                        .saturating_add(x.min(x2)),
                ),
                shape: Shape::Rectangle {
                    span,
                    remaining: span,
                    rows: y.abs_diff(y2).saturating_add(1),
                },
            })
        }
    }
    fn advance(&mut self, current: u32) {
        match &mut self.shape {
            Shape::Rectangle {
                span,
                remaining,
                rows,
            } => {
                *remaining = remaining.saturating_sub(1);
                self.current = if *remaining > 0 {
                    current.checked_add(1)
                } else {
                    *rows = rows.saturating_sub(1);
                    *remaining = *span;
                    (*rows > 0).then(|| {
                        current
                            .saturating_add(self.size.width())
                            .saturating_add(1)
                            .saturating_sub(*span)
                    })
                };
            }
            Shape::Diagonal {
                x,
                y,
                a,
                b,
                end_a,
                end_b,
            } => loop {
                if end_a.abs() == 1 {
                    *a = 0;
                    *b = if *end_b > 0 {
                        b.saturating_add(2).min(*end_b)
                    } else {
                        b.saturating_sub(2).max(*end_b)
                    };
                } else {
                    *a = a.saturating_add(if *end_a > 0 { 2 } else { -2 });
                    let new_line = if *end_a > 0 {
                        *a >= *end_a
                    } else {
                        *a <= *end_a
                    };
                    if new_line {
                        *a = if a.abs() % 2 == 1 {
                            0
                        } else if *end_a > 0 {
                            1
                        } else {
                            -1
                        };
                        *b = b.saturating_add(if *end_b > 0 { 1 } else { -1 });
                    }
                }
                if *b == *end_b {
                    self.current = None;
                    break;
                }
                let tile_x = x.saturating_add(a.saturating_sub(*b) / 2);
                let tile_y = y.saturating_add(b.saturating_add(*a) / 2);
                if tile_x >= 0
                    && tile_y >= 0
                    && tile_x < i64::from(self.size.width())
                    && tile_y < i64::from(self.size.height())
                {
                    self.current = u32::try_from(
                        tile_y
                            .saturating_mul(i64::from(self.size.width()))
                            .saturating_add(tile_x),
                    )
                    .ok();
                    break;
                }
            },
        }
    }
}
impl Iterator for Tiles {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.advance(current);
        Some(current)
    }
}
