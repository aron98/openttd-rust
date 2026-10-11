//! House-accepted final delivery and delayed `CargoPayment` accumulation.
use super::{State, WorldTickError, income, records, unsupported};
use crate::content::CargoSpec;
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

pub(super) struct Delivery<'a> {
    pub station: u32,
    pub owner: u32,
    pub cargo: u8,
    pub tile: u32,
    pub payment: u32,
    pub spec: &'a CargoSpec,
}
impl Delivery<'_> {
    pub(super) fn pay(
        &self,
        state: &mut State<'_>,
        packet: u32,
        count: u32,
    ) -> Result<(), WorldTickError> {
        for chunk in [b"INDY", b"SUBS", b"CMDL", b"CMPU"] {
            if state.table(chunk)?.records().next().is_some() {
                return Err(unsupported(
                    "cargo_delivery",
                    "industry, subsidy or active monitor consumer",
                ));
            }
        }
        let normal = records::normal(state, self.station)?;
        let accepted = if normal.number("always_accepted")? & (1_u64 << self.cargo) != 0 {
            count
        } else {
            0
        };
        let town = u32::try_from(
            normal
                .child("base")?
                .number("town")?
                .checked_sub(1)
                .ok_or_else(|| unsupported("cargo_delivery", "station town"))?,
        )
        .map_err(|_| unsupported("cargo_delivery", "town id"))?;
        if accepted > 0 {
            let status = records::goods(state, self.station, self.cargo)?.number("status")?;
            records::goods_field(
                state,
                (self.station, self.cargo),
                ("status", WireValue::Unsigned(status | 4 | 16 | 32)),
            )?;
        }
        let economy = records::row(state, *b"PLYR", self.owner)?.child("cur_economy")?;
        let WireValue::Array(delivered) =
            crate::world_access::row_field(economy.schema, economy.record, "delivered_cargo")?
        else {
            return Err(unsupported("cargo_delivery", "company delivery array"));
        };
        let Some(WireValue::Unsigned(old)) = delivered.get(usize::from(self.cargo)) else {
            return Err(unsupported("cargo_delivery", "company cargo slot"));
        };
        let total = u32::try_from(*old)
            .map_err(|_| unsupported("cargo_delivery", "company delivered width"))?
            .wrapping_add(accepted);
        state.apply(WorldEdit::Field {
            chunk: *b"PLYR",
            record: self.owner,
            path: vec![
                PathElement::Field("cur_economy".into()),
                PathElement::Index(0),
                PathElement::Field("delivered_cargo".into()),
                PathElement::Index(usize::from(self.cargo)),
            ],
            value: WireValue::Unsigned(u64::from(total)),
        })?;
        let row = records::row(state, *b"CITY", town)?;
        let (schema, received) = row.rows("received")?;
        let effect = usize::from(self.spec.town_acceptance_effect);
        let row = crate::runtime::stations::saved::StationRow {
            schema,
            record: received
                .get(effect)
                .ok_or_else(|| unsupported("cargo_delivery", "town effect"))?,
        };
        let received = row.number("new_act")?.wrapping_add(u64::from(accepted)) & 65535;
        state.apply(WorldEdit::Field {
            chunk: *b"CITY",
            record: town,
            path: vec![
                PathElement::Field("received".into()),
                PathElement::Index(effect),
                PathElement::Field("new_act".into()),
            ],
            value: WireValue::Unsigned(received),
        })?;
        let distance = income::distance(state, packet, self.tile)?;
        let age = u16::try_from(state.number(b"CAPA", packet, "periods_in_transit")?)
            .map_err(|_| unsupported("cargo_delivery", "packet age"))?;
        let profit = income::calculate(self.spec, accepted, distance, age)?;
        let divisor = std::num::NonZeroI64::new(state.number(b"CAPA", packet, "count")?)
            .ok_or_else(|| unsupported("cargo_delivery", "empty packet"))?;
        let feeder = state
            .number(b"CAPA", packet, "feeder_share")?
            .saturating_mul(i64::from(count))
            .checked_div(divisor.get())
            .ok_or_else(|| unsupported("cargo_delivery", "feeder division"))?;
        for (field, value) in [
            ("route_profit", profit),
            ("visual_profit", profit.saturating_sub(feeder)),
        ] {
            state.set_number(
                b"CAPY",
                self.payment,
                field,
                state
                    .number(b"CAPY", self.payment, field)?
                    .saturating_add(value),
            )?;
        }
        Ok(())
    }
}
