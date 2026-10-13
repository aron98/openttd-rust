use super::{
    load_currency::{CurrencyOwners, CurrencyState},
    load_string_mapping::map_string,
    load_strings::{Definition, StringKey, StringTable},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn currency_name_is_undefined_until_finalization() -> Result {
    let mut owners = CurrencyState::default();
    let mut strings = StringTable::default();
    owners.queue(7, 0, 0xd800);
    assert_eq!(owners.owners.entries.first().ok_or("owner")?.name, 2);
    assert!(
        owners
            .owners
            .entries
            .first()
            .ok_or("owner")?
            .code
            .is_empty()
    );
    let id = strings.define(
        Definition {
            key: StringKey {
                grfid: 7,
                local_id: 0xd800,
            },
            language: 1,
            new_scheme: true,
            default_id: 2,
        },
        |_, _| Ok::<_, std::io::Error>(b"Currency".to_vec()),
    )?;
    assert_eq!(owners.owners.entries.first().ok_or("owner")?.name, 2);
    owners.finalize(&strings);
    assert_eq!(owners.owners.entries.first().ok_or("owner")?.name, id);
    assert!(owners.pending.is_empty());
    Ok(())
}

#[test]
fn repeated_currency_destination_keeps_queue_order() -> Result {
    let mut state = CurrencyState::default();
    let mut strings = StringTable::default();
    strings.define(
        Definition {
            key: StringKey {
                grfid: 7,
                local_id: 0xd800,
            },
            language: 1,
            new_scheme: true,
            default_id: 2,
        },
        |_, _| Ok::<_, std::io::Error>(b"first".to_vec()),
    )?;
    state.queue(7, 0, 0xd800);
    state.queue(8, 0, 0xd800);
    state.finalize(&strings);
    assert_eq!(state.owners.entries.first().ok_or("owner")?.name, 2);
    state.finalize(&strings);
    assert_eq!(state.owners.entries.first().ok_or("owner")?.name, 2);
    Ok(())
}

#[test]
fn currency_indices_narrow_before_native_conversion() -> Result {
    let mut state = CurrencyState::default();
    state.queue(7, 256, 0);
    state.queue(7, 65535, 0);
    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.owners.entries.first().ok_or("owner")?.name, 2);
    for index in [7, 15, 18] {
        assert_eq!(
            state.owners.entries.get(index).ok_or("owner")?,
            CurrencyOwners::default()
                .entries
                .get(index)
                .ok_or("owner")?
        );
    }
    Ok(())
}

#[test]
fn reset_preserves_only_custom_including_stale_name() -> Result {
    let mut state = CurrencyOwners::default();
    state.entries.first_mut().ok_or("owner")?.name = 0x20000;
    state.entries.get_mut(31).ok_or("owner")?.name = 0x20001;
    state.entries.get_mut(31).ok_or("owner")?.code.clear();
    let fresh = CurrencyOwners::reset(Some(state.entries.get(31).ok_or("owner")?));
    assert_eq!(
        fresh.entries.first().ok_or("owner")?,
        CurrencyOwners::default().entries.first().ok_or("owner")?
    );
    assert_eq!(
        fresh.entries.get(31).ok_or("owner")?,
        state.entries.get(31).ok_or("owner")?
    );
    let strings = StringTable::default();
    assert!(
        strings
            .translation(fresh.entries.get(31).ok_or("owner")?.name, 1)
            .is_err()
    );
    Ok(())
}

#[test]
fn currency_mapping_unknown_and_custom_are_distinct() {
    let strings = StringTable::default();
    assert_eq!(map_string(&strings, 7, 0), 1);
    assert_eq!(map_string(&strings, 7, 0xd800), 2);
    assert_eq!(map_string(&strings, 7, 0xd400), 2);
    assert_eq!(map_string(&strings, 7, 0xffff), 2);
    assert_eq!(map_string(&strings, 7, 0x00ff), 1);
}

#[test]
fn all_currency_byte_ids_preserve_unreachable_owners() -> Result {
    let mut state = CurrencyState::default();
    for index in 0..256 {
        state.queue(7, index, 0);
    }
    assert_eq!(state.pending.len(), 46);
    let defaults = CurrencyOwners::default();
    for index in [7, 15, 18] {
        assert_eq!(state.owners.entries.get(index), defaults.entries.get(index));
    }
    state.finalize(&StringTable::default());
    assert_eq!(state.owners.entries.get(45).ok_or("last currency")?.name, 1);
    Ok(())
}

#[test]
fn callback_currency_mapping_masks_only_the_native_alias_bit() -> Result {
    let mut strings = StringTable::default();
    let id = strings.define(
        Definition {
            key: StringKey {
                grfid: 7,
                local_id: 0xd000,
            },
            language: 1,
            new_scheme: true,
            default_id: 2,
        },
        |_, _| Ok::<_, std::io::Error>(b"callback".to_vec()),
    )?;
    assert_eq!(map_string(&strings, 7, 0xd400), id);
    assert_eq!(map_string(&strings, 8, 0xd400), 2);
    assert_eq!(map_string(&strings, 7, 0xd800), 2);
    assert_eq!(map_string(&strings, 7, 0x000e), 4);
    assert_eq!(map_string(&strings, 7, 0x004e), 0x8a);
    Ok(())
}
