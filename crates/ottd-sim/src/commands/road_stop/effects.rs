use super::{BuildRoadStop, record};
use crate::{
    commands::CommandError,
    runtime::stations::saved,
    world_access::{field_edit, signed, unsigned},
};
use ottd_save::{
    TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};

pub(super) fn normal(station: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
fn base(station: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field("base".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
pub(super) struct Addition {
    pub station: u32,
    pub stop: u32,
    pub tile: u32,
    pub company: u8,
    pub args: BuildRoadStop,
}
impl Addition {
    pub(super) fn new_station(
        &self,
        world: &World,
        edits: &mut Vec<WorldEdit>,
    ) -> Result<(), CommandError> {
        let town = super::super::road_depot::record::nearest_town(world, self.tile)?.ok_or(
            CommandError::Unsupported("station construction without town"),
        )?;
        let name = super::naming::name(world, self.tile, town)?;
        let mut rng = ottd_core::Randomizer::from_state([
            u32::try_from(unsigned(world, b"DATE", 0, "random_state[0]")?)
                .map_err(|_| CommandError::Overflow("RNG"))?,
            u32::try_from(unsigned(world, b"DATE", 0, "random_state[1]")?)
                .map_err(|_| CommandError::Overflow("RNG"))?,
        ]);
        let random = u16::try_from(rng.next_u32() & 65535)
            .map_err(|_| CommandError::Overflow("station random"))?;
        let schema = world
            .tables()
            .get(b"STNN")
            .ok_or(CommandError::Unsupported("STNN"))?
            .schema();
        let birth = record::StationBirth {
            tile: self.tile,
            town,
            name,
            company: self.company,
            date: signed(world, b"DATE", 0, "date")?,
            random,
            stop: self.stop,
            bus: self.args.stop_type == 0,
        };
        edits.push(WorldEdit::InsertRecord {
            chunk: *b"STNN",
            record: self.station,
            value: birth.record(schema)?,
        });
        let ratings = unsigned(world, b"CITY", town, "have_ratings")? | (1_u64 << self.company);
        edits.push(field_edit(
            *b"CITY",
            town,
            "have_ratings",
            WireValue::Unsigned(ratings),
        ));
        for (field, value) in ["random_state[0]", "random_state[1]"]
            .into_iter()
            .zip(rng.state())
        {
            edits.push(field_edit(
                *b"DATE",
                0,
                field,
                WireValue::Unsigned(u64::from(value)),
            ));
        }
        Ok(())
    }
    pub(super) fn join(
        &self,
        world: &World,
        edits: &mut Vec<WorldEdit>,
    ) -> Result<(), CommandError> {
        let row = saved::row(world, *b"STNN", self.station)?.child("normal")?;
        let base_row = row.child("base")?;
        if base_row.number("facilities")? == 0 {
            return Err(CommandError::Unsupported("joining deleted station"));
        }
        let mask = if self.args.stop_type == 0 { 4 } else { 2 };
        let facilities = base_row.number("facilities")? | mask;
        edits.extend([
            field_edit(
                *b"STNN",
                self.station,
                "facilities",
                WireValue::Unsigned(facilities),
            ),
            base(self.station, "facilities", WireValue::Unsigned(facilities)),
            base(
                self.station,
                "owner",
                WireValue::Unsigned(u64::from(self.company)),
            ),
            base(
                self.station,
                "build_date",
                WireValue::Signed(signed(world, b"DATE", 0, "date")?),
            ),
        ]);
        let field = if self.args.stop_type == 0 {
            "bus_stops"
        } else {
            "truck_stops"
        };
        let mut pointer = row.number(field)?;
        let mut last = None;
        let mut seen = std::collections::BTreeSet::new();
        while pointer != 0 {
            let id = u32::try_from(pointer.saturating_sub(1))
                .map_err(|_| CommandError::Overflow("road stop pointer"))?;
            if !seen.insert(id) {
                return Err(CommandError::Unsupported("cyclic road stop list"));
            }
            last = Some(id);
            pointer = saved::row(world, *b"ROAD", id)?.number("next")?;
        }
        let value = WireValue::Unsigned(u64::from(self.stop) + 1);
        edits.push(match last {
            Some(id) => field_edit(*b"ROAD", id, "next", value),
            None => normal(self.station, field, value),
        });
        Ok(())
    }
    pub(super) fn tile_and_stop(
        &self,
        world: &World,
        edits: &mut Vec<WorldEdit>,
    ) -> Result<(), CommandError> {
        let source = edits
            .iter()
            .rev()
            .find_map(|e| match e {
                WorldEdit::Tile { index, value } if *index == self.tile => Some(value),
                _ => None,
            })
            .ok_or(CommandError::Unsupported("cleared road stop tile"))?;
        let mut raw = TileRawParts::from(source);
        raw.tile_type = (raw.tile_type & 15) | 0x50;
        raw.m1 = 0x60 | self.company;
        raw.m2 = u16::try_from(self.station).map_err(|_| CommandError::Overflow("station ID"))?;
        raw.m3 = self.company << 4;
        raw.m4 = self.args.road_type;
        raw.m5 = self.args.direction;
        raw.m6 = (raw.m6 & 3) | if self.args.stop_type == 0 { 24 } else { 16 };
        raw.m7 = self.company;
        raw.m8 = 63 << 6;
        edits.push(WorldEdit::Tile {
            index: self.tile,
            value: raw.into(),
        });
        let schema = world
            .tables()
            .get(b"ROAD")
            .ok_or(CommandError::Unsupported("ROAD"))?
            .schema();
        edits.push(WorldEdit::InsertRecord {
            chunk: *b"ROAD",
            record: self.stop,
            value: record::stop(schema, self.tile)?,
        });
        Ok(())
    }
}
