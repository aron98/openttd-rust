//! Stable structural index ordering and native invalid-parent recovery.
#![cfg(test)]
use ottd_save::{
    Savegame, TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn set(schema: &TableSchema, row: &mut TableRecord, name: &str, value: u64) -> Result {
    let position = schema
        .fields()
        .iter()
        .position(|f| f.name() == name)
        .ok_or("field")?;
    *row.values_mut().get_mut(position).ok_or("value")? = WireValue::Unsigned(value);
    Ok(())
}
#[test]
fn indexes_group_children_in_id_order_and_ignores_invalid_parent_owners() -> Result {
    let original = world()?;
    let mut save = original.to_savegame()?;
    let mut groups = original.tables().get(b"GRPS").ok_or("groups")?.clone();
    let schema = groups.schema().clone();
    let template = groups.records().get(&0).ok_or("group")?.clone();
    for (id, owner, parent) in [(9, 1, 0), (2, 1, 0), (5, 0, 0), (12, 1, 63999)] {
        let mut record = template.clone();
        set(&schema, &mut record, "owner", owner)?;
        set(&schema, &mut record, "parent", parent)?;
        groups.records_mut().insert(id, record);
    }
    save.replace_chunk(groups.encode()?)?;
    let restored = World::decode(&save)?;
    let groups = &restored.derived().groups;
    assert_eq!(
        groups.iter().map(|g| g.id).collect::<Vec<_>>(),
        vec![0, 2, 5, 9, 12]
    );
    assert_eq!(groups.first().ok_or("root")?.children, vec![2, 9]);
    assert!(
        groups
            .iter()
            .filter(|g| g.id != 0)
            .all(|g| g.children.is_empty())
    );
    Ok(())
}
fn vehicle_field(id: u32, name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    }
}
#[test]
fn indexes_order_membership_without_sorting_away_shared_chain_order() -> Result {
    let mut world = world()?;
    world.edit_batch(vec![
        vehicle_field(13, "orders", 1),
        vehicle_field(13, "next_shared", 13),
    ])?;
    let lists = &world.derived().order_lists;
    assert_eq!(
        lists
            .iter()
            .find(|list| list.id == 0)
            .ok_or("list0")?
            .vehicles,
        vec![13, 12]
    );
    assert_eq!(
        lists
            .iter()
            .find(|list| list.id == 1)
            .ok_or("list1")?
            .vehicles,
        vec![20]
    );
    assert_eq!(
        lists
            .iter()
            .find(|list| list.id == 2)
            .ok_or("list2")?
            .vehicles,
        vec![21]
    );
    Ok(())
}
