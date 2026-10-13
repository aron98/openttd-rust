//! Packet construction and station append preserve native allocation-before-merge history.
use super::{CargoRuntime, RuntimeError, packet, write};
use crate::{
    WorldTickError, WorldTickState as State, cargo::CargoSource, unsupported_tick as unsupported,
};
use ottd_save::{TableRecord, WireValue, world::WorldEdit};

impl CargoRuntime {
    pub(in crate::runtime) fn append_production(
        &mut self,
        state: &mut State<'_>,
        station: u32,
        cargo: u8,
        amount: u16,
        source: CargoSource,
    ) -> Result<(), WorldTickError> {
        if amount == 0 {
            return Err(unsupported("cargo_production", "zero packet count"));
        }
        let id = self.pool.allocate().map_err(RuntimeError::from)?;
        let (source_type, source_id) = match source {
            CargoSource::Town(id) => (1, id),
            CargoSource::Industry(id) => (0, id),
        };
        let table = state.table(b"CAPA")?;
        let mut values = Vec::new();
        for field in table.schema().fields() {
            values.push(match field.name() {
                "count" => WireValue::Unsigned(u64::from(amount)),
                "source" => WireValue::Unsigned(u64::from(station)),
                "source_type" => WireValue::Unsigned(source_type),
                "source_id" => WireValue::Unsigned(u64::from(source_id)),
                "source_xy" => WireValue::Unsigned(u64::from(u32::MAX)),
                "loaded_at_xy" => WireValue::Unsigned(u64::from(u16::MAX)),
                "periods_in_transit" => WireValue::Unsigned(0),
                "travelled.x" | "travelled.y" | "feeder_share" => WireValue::Signed(0),
                _ => return Err(unsupported("cargo_production", "packet constructor schema")),
            });
        }
        state.apply(WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: id,
            value: TableRecord::new(values),
        })?;
        let mut list = self
            .stations
            .get(&station)
            .and_then(|goods| goods.get(&u32::from(cargo)))
            .cloned()
            .unwrap_or_default();
        list.count = list.count.wrapping_add(u32::from(amount));
        let packets = list.packets.entry(u32::from(u16::MAX)).or_default();
        let mut merged = false;
        for existing in packets.iter().rev() {
            if packet::merge(state, self, *existing, id)? {
                merged = true;
                break;
            }
        }
        if !merged {
            packets.push(id);
        }
        write::station(state, station, cargo, &list)?;
        self.stations
            .get_mut(&station)
            .ok_or(RuntimeError::Invalid("station cargo cache"))?
            .insert(u32::from(cargo), list);
        Ok(())
    }
}
