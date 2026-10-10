use super::{
    load_currency_native::{Result, setup},
    load_currency_properties_api_cases::index_cases,
    load_currency_properties_native::{Operation, compare_api},
};
use serde_json::json;

#[test]
#[ignore = "measured first-case pilot only; does not admit the complete property corpus"]
fn original_currency_property_first_case() -> Result {
    let (name, operations) = index_cases().into_iter().next().ok_or("first API case")?;
    assert_eq!(name, "indices-rate");
    assert_eq!(operations.len(), 512);
    for (position, operation) in operations.iter().enumerate() {
        match operation {
            Operation::Property {
                grfid,
                property,
                first,
                count,
                reserve,
                raw,
            } => {
                assert_eq!(*grfid, 7);
                assert_eq!(*property, 0x0b);
                assert_eq!(usize::from(*first), position / 2);
                assert_eq!(*count, 1);
                assert_eq!(*reserve, position % 2 == 0);
                assert_eq!(raw.as_slice(), 65_536_000_u32.to_le_bytes());
            }
            Operation::ResetCurrencies { .. } => panic!("unexpected first-case reset"),
        }
    }
    let (root, directory, oracle, pack) = setup()?;
    let case = directory.join(name);
    std::fs::create_dir(&case)?;
    let controls = compare_api(&root, &case, &oracle, &pack, &operations)?;
    assert_eq!(controls, 281_089);
    std::fs::write(
        directory.join("first-case-summary.json"),
        serde_json::to_vec(&json!({
            "scope":"first-case-pilot-only",
            "complete_corpus":false,
            "case":name,
            "operations":512,
            "reserve_operations":256,
            "activation_operations":256,
            "controls":controls
        }))?,
    )?;
    println!(
        "first-case pilot only: 512 rate operations, {controls} controls; complete corpus not admitted"
    );
    Ok(())
}
