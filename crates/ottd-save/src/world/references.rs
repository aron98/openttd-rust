use super::{
    MapState, Row, TableChunk, TableRecord, TableSchema, WireValue, WorldError, invalid, name, rows,
};
use std::collections::{BTreeMap, BTreeSet};

type Tables = BTreeMap<[u8; 4], TableChunk>;

pub(super) fn validate(tables: &Tables, map: &MapState) -> Result<(), WorldError> {
    for (chunk, table) in tables {
        let limit = match chunk {
            b"VEHS" | b"CAPY" | b"PSAC" => 0x000f_f000,
            b"CAPA" => 0x00ff_f000,
            b"OBJS" => 0x00ff_0000,
            b"PLYR" | b"AIPL" => 15,
            b"CITY" | b"INDY" | b"STNN" | b"ROAD" | b"ORDL" | b"GRPS" | b"ERNW" | b"ENGN"
            | b"SIGN" | b"STPE" | b"STPA" | b"LEAE" | b"DEPT" | b"GOAL" => 64000,
            b"BKOR" | b"LEAT" => 255,
            b"SUBS" => 256,
            b"LGRP" | b"LGRJ" => 65535,
            _ => u32::MAX,
        };
        for (id, record) in table.records() {
            let path = format!("{}[{id}]", name(*chunk));
            if *id >= limit {
                return Err(invalid(&path, "pool ID out of range"));
            }
            validate_shapes(*chunk, table.schema(), record, &path)?;
        }
    }
    validate_variants(tables)?;
    validate_vehicle_edges(tables)?;
    for (chunk, field) in [
        (*b"VEHS", "next"),
        (*b"VEHS", "next_shared"),
        (*b"ROAD", "next"),
        (*b"ERNW", "next"),
    ] {
        let mut links = BTreeMap::new();
        for (id, row) in rows(tables, chunk)? {
            let node = if chunk == *b"VEHS" {
                let (kind, common) = vehicle_row(row)?;
                if field == "next_shared" && kind >= 4 {
                    None
                } else {
                    common
                }
            } else {
                Some(row)
            };
            if let Some(row) = node {
                links.insert(id, row.reference(field)?);
            }
        }
        validate_chain(&links, &name(chunk))?;
    }
    for (index, tile) in map.tiles().iter().enumerate() {
        let object = match tile.tile_type() >> 4 {
            3 => Some((*b"CITY", u32::from(tile.m2()))),
            5 => Some((*b"STNN", u32::from(tile.m2()))),
            8 => Some((*b"INDY", u32::from(tile.m2()))),
            10 => Some((
                *b"OBJS",
                u32::from(tile.m2()) | (u32::from(tile.m5()) << 16),
            )),
            0..=2 | 4 | 6 | 7 | 9 => None,
            _ => return Err(invalid(&format!("map[{index}]"), "invalid tile kind")),
        };
        if let Some((pool, id)) = object {
            require(tables, pool, id, &format!("map[{index}]"))?;
        }
    }
    Ok(())
}
fn validate_shapes(
    chunk: [u8; 4],
    schema: &TableSchema,
    row: &TableRecord,
    path: &str,
) -> Result<(), WorldError> {
    for (field, value) in schema.fields().iter().zip(row.values()) {
        let path = format!("{path}/{}", field.name());
        cardinality(chunk, field.name(), value, &path)?;
        if let WireValue::Structs(records) = value {
            let child = field
                .child()
                .ok_or_else(|| invalid(&path, "missing child schema"))?;
            for (index, record) in records.iter().enumerate() {
                validate_shapes(chunk, child, record, &format!("{path}[{index}]"))?;
            }
        }
    }
    Ok(())
}
pub(super) fn require(
    tables: &Tables,
    pool: [u8; 4],
    id: u32,
    path: &str,
) -> Result<(), WorldError> {
    if !tables
        .get(&pool)
        .is_some_and(|t| t.records().contains_key(&id))
    {
        return Err(invalid(path, "dangling object reference"));
    }
    Ok(())
}

pub(super) fn validate_chain(
    links: &BTreeMap<u32, Option<u32>>,
    path: &str,
) -> Result<(), WorldError> {
    let mut predecessors = BTreeSet::new();
    for next in links.values().flatten() {
        if !links.contains_key(next) {
            return Err(invalid(path, "chain points outside its domain"));
        }
        if !predecessors.insert(*next) {
            return Err(invalid(path, "multiple chain predecessors"));
        }
    }
    let mut visited = BTreeSet::new();
    for head in links.keys().filter(|id| !predecessors.contains(id)) {
        let mut current = Some(*head);
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(invalid(path, "cyclic chain"));
            }
            current = links.get(&id).copied().flatten();
        }
    }
    if visited.len() != links.len() {
        return Err(invalid(path, "cyclic chain"));
    }
    Ok(())
}

pub(super) fn vehicle_row(row: Row<'_>) -> Result<(u8, Option<Row<'_>>), WorldError> {
    let kind =
        u8::try_from(row.unsigned("type")?).map_err(|_| invalid("VEHS/type", "invalid variant"))?;
    let field = match kind {
        0 => "train",
        1 => "roadveh",
        2 => "ship",
        3 => "aircraft",
        4 => "effect",
        5 => "disaster",
        _ => return Err(invalid("VEHS/type", "invalid variant")),
    };
    let variant = row.single(field)?;
    let common = match kind {
        0..=3 => Some(variant.single("common")?),
        4 => None,
        5 => Some(variant),
        _ => return Err(invalid("VEHS/type", "invalid variant")),
    };
    Ok((kind, common))
}
fn validate_variants(tables: &Tables) -> Result<(), WorldError> {
    for (id, row) in rows(tables, *b"VEHS")? {
        let (kind, common) = vehicle_row(row)?;
        for (index, name) in ["train", "roadveh", "ship", "aircraft", "effect", "disaster"]
            .iter()
            .enumerate()
        {
            if row.child(name)?.len() != usize::from(usize::from(kind) == index) {
                return Err(invalid(
                    &format!("VEHS[{id}]"),
                    "inconsistent vehicle variant structures",
                ));
            }
        }
        if kind < 4 {
            let common = common.ok_or_else(|| invalid("VEHS", "missing vehicle common"))?;
            let owner = u32::try_from(common.unsigned("owner")?)
                .map_err(|_| invalid("VEHS/owner", "invalid owner"))?;
            require(tables, *b"PLYR", owner, "VEHS/owner")?;
            let group = u32::try_from(common.unsigned("group_id")?)
                .map_err(|_| invalid("VEHS/group_id", "invalid group"))?;
            if group != 65534 {
                require(tables, *b"GRPS", group, "VEHS/group_id")?;
                let groups = tables
                    .get(b"GRPS")
                    .ok_or_else(|| invalid("VEHS/group_id", "missing group pool"))?;
                let record = groups
                    .records()
                    .get(&group)
                    .ok_or_else(|| invalid("VEHS/group_id", "missing group"))?;
                let group = Row {
                    schema: groups.schema(),
                    record,
                };
                if group.unsigned("owner")? != u64::from(owner)
                    || group.unsigned("vehicle_type")? != u64::from(kind)
                {
                    return Err(invalid(
                        "VEHS/group_id",
                        "group owner or vehicle type differs",
                    ));
                }
            }
        }
    }
    for (id, row) in rows(tables, *b"STNN")? {
        let waypoint = row.unsigned("facilities")? & 0x40 != 0;
        if row.child("normal")?.len() != usize::from(!waypoint)
            || row.child("waypoint")?.len() != usize::from(waypoint)
        {
            return Err(invalid(
                &format!("STNN[{id}]"),
                "inconsistent station variant structures",
            ));
        }
        let base = row
            .single(if waypoint { "waypoint" } else { "normal" })?
            .single("base")?;
        if base.unsigned("facilities")? != row.unsigned("facilities")? {
            return Err(invalid(
                "STNN/facilities",
                "variant disagrees with base facilities",
            ));
        }
    }
    Ok(())
}

fn cardinality(
    chunk: [u8; 4],
    field: &str,
    value: &WireValue,
    path: &str,
) -> Result<(), WorldError> {
    if let WireValue::Structs(rows) = value {
        let singleton = matches!(
            (&chunk, field),
            (b"PLYR", "settings" | "cur_economy")
                | (b"LGRJ", "linkgraph")
                | (b"VEHS", "common")
                | (b"STNN", "base")
        );
        if singleton && rows.len() != 1 {
            return Err(invalid(path, "expected one structure"));
        }
        let maximum = match (&chunk, field) {
            (
                b"GLOG",
                "mode" | "revision" | "oldver" | "setting" | "grfadd" | "grfrem" | "grfcompat"
                | "grfparam" | "grfmove" | "grfbug" | "emergency",
            ) => 1,
            (b"STNN", "goods") => 64,
            (b"INDY", "accepted" | "produced") => 16,
            (b"PLYR", "liveries") => 23,
            (b"PLYR", "old_economy") => 24,
            _ => usize::MAX,
        };
        if rows.len() > maximum {
            return Err(invalid(path, "too many nested records"));
        }
    }
    Ok(())
}

fn validate_vehicle_edges(tables: &Tables) -> Result<(), WorldError> {
    let vehicles: BTreeMap<_, _> = rows(tables, *b"VEHS")?
        .map(|(id, row)| Ok((id, vehicle_row(row)?)))
        .collect::<Result<_, WorldError>>()?;
    for (kind, common) in vehicles.values() {
        let Some(common) = common else {
            continue;
        };
        for field in ["next", "next_shared"] {
            if field == "next_shared" && *kind >= 4 {
                continue;
            }
            if let Some(next) = common.reference(field)? {
                let (next_kind, next_common) = vehicles
                    .get(&next)
                    .ok_or_else(|| invalid(field, "dangling vehicle"))?;
                if kind != next_kind {
                    return Err(invalid(field, "linked vehicles have different types"));
                }
                if *kind < 4 {
                    let next_common =
                        next_common.ok_or_else(|| invalid(field, "missing common state"))?;
                    if common.unsigned("owner")? != next_common.unsigned("owner")? {
                        return Err(invalid(field, "linked vehicles have different owners"));
                    }
                    if field == "next_shared"
                        && (common.reference("orders")?.is_none()
                            || common.reference("orders")? != next_common.reference("orders")?)
                    {
                        return Err(invalid(field, "shared vehicles have different order lists"));
                    }
                }
            }
        }
    }
    Ok(())
}
