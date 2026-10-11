use super::{
    input::Part,
    types::{ConsistError, ConsistRefusal},
};
use crate::runtime::VehicleId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn chain(parts: &BTreeMap<VehicleId, Part>) -> Result<Vec<Part>, ConsistError> {
    let mut incoming = BTreeSet::new();
    for part in parts.values() {
        if let Some(next) = part.next {
            if !parts.contains_key(&next) || !incoming.insert(next) {
                return Err(ConsistRefusal::Graph.into());
            }
        }
    }
    let roots: Vec<_> = parts
        .keys()
        .filter(|id| !incoming.contains(id))
        .copied()
        .collect();
    let [root] = roots.as_slice() else {
        return Err(ConsistRefusal::MultipleRoots.into());
    };
    let mut result = Vec::with_capacity(parts.len());
    let mut visited = BTreeSet::new();
    let mut current = Some(*root);
    while let Some(id) = current {
        if !visited.insert(id) {
            return Err(ConsistRefusal::Graph.into());
        }
        let part = *parts.get(&id).ok_or(ConsistRefusal::Graph)?;
        let valid_subtype = match part.spec.railveh_type {
            0 => part.subtype == if id == *root { 9 } else { 8 },
            1 => {
                if id == *root {
                    part.subtype == 41
                } else {
                    matches!(part.subtype, 40 | 32)
                }
            }
            2 => id != *root && part.subtype == 4,
            _ => false,
        };
        if !valid_subtype {
            return Err(ConsistRefusal::Subtype.into());
        }
        let rear = part.spec.railveh_type == 1 && part.subtype & 8 == 0;
        if part.sprite != part.spec.image_index.wrapping_add(u8::from(rear)) {
            return Err(ConsistRefusal::Multihead.into());
        }
        current = part.next;
        result.push(part);
    }
    if visited.len() != parts.len() {
        return Err(ConsistRefusal::Graph.into());
    }
    Ok(result)
}

pub(super) fn pairs(chain: &[Part]) -> Result<BTreeMap<VehicleId, VehicleId>, ConsistError> {
    let mut pairs = BTreeMap::new();
    for (index, part) in chain.iter().enumerate() {
        if part.spec.railveh_type != 1 || pairs.contains_key(&part.id) {
            continue;
        }
        if part.subtype & 8 == 0 {
            return Err(ConsistRefusal::Multihead.into());
        }
        let partner = chain
            .iter()
            .skip(index.saturating_add(1))
            .find(|next| {
                next.engine == part.engine
                    && next.spec.railveh_type == 1
                    && !pairs.contains_key(&next.id)
            })
            .ok_or(ConsistRefusal::Multihead)?;
        if partner.subtype != 32 {
            return Err(ConsistRefusal::Multihead.into());
        }
        pairs.insert(part.id, partner.id);
        pairs.insert(partner.id, part.id);
    }
    Ok(pairs)
}
