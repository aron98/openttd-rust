//! Public runtime identity and restoration error vocabulary.
use super::pools;
use crate::{WorldTickError, content::ContentError, unsupported_tick};
use serde::Serialize;

/// Native sparse vehicle pool identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct VehicleId(pub(super) u32);
impl VehicleId {
    /// Wrap a pool index; lookups check whether it exists.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Native pool index, not a saved pointer encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Rejected or malformed runtime restoration input.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Invalid native vehicle allocation metadata.
    #[error(transparent)]
    Pool(#[from] pools::PoolError),
    /// Saved order backups require an explicit host load context.
    #[error("loaded order backups require explicit runtime context")]
    OrderContextRequired,
    /// Pinned native deletion would assert because a loaded object lost its pool index.
    #[error("native backup deletion invariant: physical slot {slot} has object index {index}")]
    NativeBackupIndex {
        /// Authoritative physical pool slot / saved row ID.
        slot: u32,
        /// Original runtime object's index after constructor/load.
        index: u8,
    },
    /// Invalid saved transaction.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
    /// Unsupported content configuration.
    #[error(transparent)]
    Content(#[from] ContentError),
    /// Feature requiring a later runtime family.
    #[error("unsupported runtime: {0}")]
    Unsupported(&'static str),
    /// Missing or invalid native saved field.
    #[error("invalid runtime input: {0}")]
    Invalid(&'static str),
}

/// Native `MAX_NEWS_AMOUNT` (`news_gui.cpp`); trimming stays refused, so the
/// queue only ever holds single inserts in the admitted domain.
const MAX_NEWS_AMOUNT: usize = 1 << 10;

/// One native `AddVehicleAdviceNewsItem(AdviceType::VehicleUnprofitable, ...)`
/// item; the encoded headline is exactly the probed
/// (company, vehicle, string, display profit) tuple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewsItem {
    /// Owning company of the vehicle (`v->owner`).
    company: u8,
    /// Native vehicle pool index (`v->index`).
    vehicle: u32,
    /// Selected native string id.
    string: u32,
    /// Display profit (`profit_this_year >> 8`) encoded into the headline.
    profit: i64,
}
impl NewsItem {
    /// Build the unprofitable-vehicle advice item the native branch inserts.
    pub(crate) const fn unprofitable(company: u8, vehicle: u32, string: u32, profit: i64) -> Self {
        Self {
            company,
            vehicle,
            string,
            profit,
        }
    }
}

/// Process-owned native `std::list _news` for vehicle advice items.
///
/// Native saveload has no news chunk: news state is never serialized and a
/// same-process reload keeps the old queue, exactly like the company rating
/// slots. The client `vehicle_income_warn` setting is process state at its
/// native default; the sim reads no configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewsQueue {
    /// Items in native front-first order (`_news.emplace_front`).
    items: Vec<NewsItem>,
    /// Process-owned `_settings_client.gui.vehicle_income_warn` default.
    vehicle_income_warn: bool,
}
impl NewsQueue {
    /// Initialize the empty native queue with the client setting default.
    pub const fn fresh_process() -> Self {
        Self {
            items: Vec::new(),
            vehicle_income_warn: true,
        }
    }
    /// Front-insert one advice item, mirroring `AddNewsItem`'s `emplace_front`.
    pub(crate) fn insert(&mut self, item: NewsItem) -> Result<(), WorldTickError> {
        self.items.insert(0, item);
        if self.items.len() > MAX_NEWS_AMOUNT {
            return Err(unsupported_tick(
                "vehicle_year",
                "news trimming beyond the single insert",
            ));
        }
        Ok(())
    }
    /// Whether the process-owned client setting admits income advice news.
    pub(crate) const fn vehicle_income_warn(&self) -> bool {
        self.vehicle_income_warn
    }
    /// Current items in native front-first order.
    pub fn items(&self) -> &[NewsItem] {
        &self.items
    }
    /// Current queue length; exported by the observer as `news_queue`.
    pub fn len(&self) -> usize {
        self.items.len()
    }
    /// Whether the queue holds no item.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_NEWS_AMOUNT, NewsItem, NewsQueue};

    fn probe_item() -> NewsItem {
        NewsItem::unprofitable(0, 0, 0x2F9, -1125)
    }

    #[test]
    fn fresh_process_owns_the_default_client_setting() {
        let queue = NewsQueue::fresh_process();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.vehicle_income_warn());
    }

    #[test]
    fn front_insert_orders_newest_first() {
        let mut queue = NewsQueue::fresh_process();
        let older = NewsItem::unprofitable(0, 0, 0x2F9, 12);
        assert!(queue.insert(older.clone()).is_ok());
        assert!(queue.insert(probe_item()).is_ok());
        assert_eq!(queue.items(), [probe_item(), older].as_slice());
    }

    #[test]
    fn serialized_item_exposes_the_native_probe_tuple() {
        let value = serde_json::to_value(NewsQueue::fresh_process()).ok();
        assert_eq!(
            value,
            Some(serde_json::json!({"vehicle_income_warn": true, "items": []}))
        );
        let mut queue = NewsQueue::fresh_process();
        assert!(queue.insert(probe_item()).is_ok());
        let value = serde_json::to_value(&queue).ok();
        assert_eq!(
            value,
            Some(serde_json::json!({"vehicle_income_warn": true, "items":
                [{"company": 0, "vehicle": 0, "string": 761, "profit": -1125}]}))
        );
    }

    #[test]
    fn trimming_beyond_the_single_insert_refuses() {
        let mut queue = NewsQueue::fresh_process();
        for index in 0..MAX_NEWS_AMOUNT {
            assert!(
                queue
                    .insert(NewsItem::unprofitable(
                        0,
                        u32::try_from(index).unwrap_or(u32::MAX),
                        0x2F9,
                        -1125,
                    ))
                    .is_ok()
            );
        }
        assert_eq!(queue.len(), MAX_NEWS_AMOUNT);
        let Err(error) = queue.insert(probe_item()) else {
            panic!("trimming must refuse");
        };
        assert!(
            error
                .to_string()
                .contains("vehicle_year: news trimming beyond the single insert"),
            "{error}"
        );
    }
}
