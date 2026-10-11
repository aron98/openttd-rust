//! Industry production uses the existing reciprocal catchment and shared station consumer.
use crate::{
    WorldTickError, WorldTickState as State,
    cargo::{CargoSource, Production, TileArea},
    runtime::transport::TransportRuntime,
    unsupported_tick as unsupported,
};

impl TransportRuntime {
    pub(in crate::runtime) fn industry_production(
        &mut self,
        state: &mut State<'_>,
        production: Production,
    ) -> Result<u32, WorldTickError> {
        let CargoSource::Industry(source) = production.source else {
            return Err(unsupported("industry_cargo", "industry producer source"));
        };
        let id = u32::from(source);
        let area = TileArea {
            tile: u32::try_from(state.number(b"INDY", id, "location.tile")?)
                .map_err(|_| unsupported("industry_cargo", "source tile"))?,
            width: u16::try_from(state.number(b"INDY", id, "location.w")?)
                .map_err(|_| unsupported("industry_cargo", "source width"))?,
            height: u16::try_from(state.number(b"INDY", id, "location.h")?)
                .map_err(|_| unsupported("industry_cargo", "source height"))?,
        };
        if production.area != area {
            return Err(unsupported("industry_cargo", "source area"));
        }
        let exclusive = u8::try_from(state.number(b"INDY", id, "exclusive_consumer")?)
            .map_err(|_| unsupported("industry_cargo", "exclusive consumer"))?;
        let nearby = self
            .cargo
            .industry_near
            .as_ref()
            .and_then(|industries| industries.get(&id))
            .ok_or_else(|| unsupported("industry_cargo", "industry recipient cache unavailable"))?
            .clone();
        self.distribute(state, production, &nearby, exclusive)
    }
}
