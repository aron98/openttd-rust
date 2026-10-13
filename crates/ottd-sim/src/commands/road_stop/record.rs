use crate::commands::CommandError;
use ottd_save::{TableRecord, TableSchema, WireValue};

pub(super) struct StationBirth {
    pub tile: u32,
    pub town: u32,
    pub name: u16,
    pub company: u8,
    pub date: i64,
    pub random: u16,
    pub stop: u32,
    pub bus: bool,
}
impl StationBirth {
    const fn facilities(&self) -> u64 {
        if self.bus { 4 } else { 2 }
    }
    pub(super) fn record(&self, schema: &TableSchema) -> Result<TableRecord, CommandError> {
        construct(schema, |field| match field.name() {
            "facilities" => Ok(WireValue::Unsigned(self.facilities())),
            "normal" => Ok(WireValue::Structs(vec![self.normal(child(field)?)?])),
            "waypoint" | "speclist" | "roadstopspeclist" | "roadstoptiledata" => {
                Ok(WireValue::Structs(Vec::new()))
            }
            _ => Err(CommandError::Unsupported("station constructor schema")),
        })
    }
    fn normal(&self, schema: &TableSchema) -> Result<TableRecord, CommandError> {
        construct(schema, |field| {
            let number = match field.name() {
                "base" => return Ok(WireValue::Structs(vec![self.base(child(field)?)?])),
                "goods" => return Ok(WireValue::Structs(vec![goods(child(field)?)?; 64])),
                "loading_vehicles" => return Ok(WireValue::Array(Vec::new())),
                "bus_stops" => {
                    if self.bus {
                        u64::from(self.stop) + 1
                    } else {
                        0
                    }
                }
                "truck_stops" => {
                    if self.bus {
                        0
                    } else {
                        u64::from(self.stop) + 1
                    }
                }
                "train_station.tile"
                | "ship_station.tile"
                | "docking_station.tile"
                | "airport.tile" => u64::from(u32::MAX),
                "indtype" | "time_since_load" | "time_since_unload" | "last_vehicle_type"
                | "airport.rotation" => 255,
                "train_station.w"
                | "train_station.h"
                | "ship_station.w"
                | "ship_station.h"
                | "docking_station.w"
                | "docking_station.h"
                | "airport.w"
                | "airport.h"
                | "airport.type"
                | "airport.layout"
                | "airport.flags"
                | "airport.psa"
                | "had_vehicle_of_type"
                | "always_accepted" => 0,
                _ => {
                    return Err(CommandError::Unsupported(
                        "normal station constructor schema",
                    ));
                }
            };
            Ok(WireValue::Unsigned(number))
        })
    }
    fn base(&self, schema: &TableSchema) -> Result<TableRecord, CommandError> {
        construct(schema, |field| {
            let number = match field.name() {
                "xy" => u64::from(self.tile),
                "town" => u64::from(self.town) + 1,
                "string_id" => u64::from(self.name),
                "owner" => u64::from(self.company),
                "facilities" => self.facilities(),
                "random_bits" => u64::from(self.random),
                "build_date" => return Ok(WireValue::Signed(self.date)),
                "name" => return Ok(WireValue::Bytes(Vec::new())),
                "delete_ctr" | "waiting_triggers" => 0,
                _ => return Err(CommandError::Unsupported("base station constructor schema")),
            };
            Ok(WireValue::Unsigned(number))
        })
    }
}
fn goods(schema: &TableSchema) -> Result<TableRecord, CommandError> {
    construct(schema, |field| {
        let number = match field.name() {
            "status"
            | "last_speed"
            | "amount_fract"
            | "cargo.reserved_count"
            | "max_waiting_cargo" => 0,
            "time_since_pickup" | "last_age" => 255,
            "rating" => 175,
            "link_graph" | "node" => 65535,
            "flow" | "cargo" => return Ok(WireValue::Structs(Vec::new())),
            _ => {
                return Err(CommandError::Unsupported(
                    "station goods constructor schema",
                ));
            }
        };
        Ok(WireValue::Unsigned(number))
    })
}
fn child(field: &ottd_save::FieldSchema) -> Result<&TableSchema, CommandError> {
    field.child().ok_or(CommandError::Unsupported(
        "station constructor child schema",
    ))
}
fn construct(
    schema: &TableSchema,
    mut field: impl FnMut(&ottd_save::FieldSchema) -> Result<WireValue, CommandError>,
) -> Result<TableRecord, CommandError> {
    Ok(TableRecord::new(
        schema
            .fields()
            .iter()
            .map(&mut field)
            .collect::<Result<_, _>>()?,
    ))
}
pub(super) fn stop(schema: &TableSchema, tile: u32) -> Result<TableRecord, CommandError> {
    construct(schema, |field| {
        Ok(WireValue::Unsigned(match field.name() {
            "xy" => u64::from(tile),
            "status" => 3,
            "next" => 0,
            _ => return Err(CommandError::Unsupported("road stop constructor schema")),
        }))
    })
}
