//! Native primary-road annual rotation after company and town yearly callbacks.
use super::{RoadPhases, State, WorldTickError, number, set, unsupported};
use serde::Serialize;

/// Native `STR_NEWS_VEHICLE_UNPROFITABLE_YEAR` (table/strings.h at the pinned
/// source). The wallclock `..._PERIOD` variant stays refused by the existing
/// movement-domain admission (`economy.timekeeping_units` must be zero).
const STR_NEWS_VEHICLE_UNPROFITABLE_YEAR: u32 = 0x2F9;
/// Native `MAX_NEWS_AMOUNT` (news_gui.cpp); trimming stays refused, so the
/// queue only ever holds single inserts in the admitted domain.
const MAX_NEWS_AMOUNT: usize = 1 << 10;
/// Native `VEHICLE_PROFIT_MIN_AGE` (vehicle_func.h).
const VEHICLE_PROFIT_MIN_AGE: i64 = 730;

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

/// Process-owned native `std::list _news` for vehicle advice items.
///
/// Native saveload has no news chunk: news state is never serialized and a
/// same-process reload keeps the old queue, exactly like the company rating
/// slots (`company_rating.rs`). The client `vehicle_income_warn` setting is
/// process state at its native default; the sim reads no configuration file.
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
            return Err(unsupported(
                "vehicle_year",
                "news trimming beyond the single insert",
            ));
        }
        Ok(())
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

/// First playable company, mirroring native single-player load
/// (`SetLocalCompany(GetFirstPlayableCompanyID())`): the lowest human company.
/// The movement domain refuses AI companies at admission, so every saved
/// company is human and the lowest pool id is the local company.
fn local_company(state: &State<'_>) -> Result<u8, WorldTickError> {
    u8::try_from(state.company_ids().into_iter().min().unwrap_or(0))
        .map_err(|_| unsupported("vehicle_year", "local company"))
}

pub(super) fn run(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let current = number(state, runner.id, false, "profit_this_year")?;
    let display = current >> 8;
    if number(state, runner.id, false, "economy_age")? >= VEHICLE_PROFIT_MIN_AGE && display < 0 {
        let owner = u8::try_from(number(state, runner.id, false, "owner")?)
            .map_err(|_| unsupported("vehicle_year", "vehicle owner"))?;
        if runner.news.vehicle_income_warn && owner == local_company(state)? {
            runner.news.insert(NewsItem {
                company: owner,
                vehicle: runner.id.raw(),
                string: STR_NEWS_VEHICLE_UNPROFITABLE_YEAR,
                profit: display,
            })?;
        }
        // Native AI branch (`AI::NewEvent`): human owners drop the event before
        // any storage, so no VM or event queue is involved. AI-company owners
        // remain refused by the movement-domain admission (`company is_ai`).
    }
    set(state, runner.id, false, "profit_last_year", current)?;
    set(state, runner.id, false, "profit_this_year", 0)?;
    runner.group_profits.recount(state.view())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_NEWS_AMOUNT, NewsItem, NewsQueue};

    fn probe_item() -> NewsItem {
        NewsItem {
            company: 0,
            vehicle: 0,
            string: 0x2F9,
            profit: -1125,
        }
    }

    #[test]
    fn fresh_process_owns_the_default_client_setting() {
        let queue = NewsQueue::fresh_process();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.vehicle_income_warn);
    }

    #[test]
    fn front_insert_orders_newest_first() {
        let mut queue = NewsQueue::fresh_process();
        let older = NewsItem {
            company: 0,
            vehicle: 0,
            string: 0x2F9,
            profit: 12,
        };
        assert!(queue.insert(older.clone()).is_ok());
        assert!(queue.insert(probe_item()).is_ok());
        assert_eq!(queue.items(), [probe_item(), older].as_slice());
    }

    #[test]
    fn serialized_item_exposes_the_native_probe_tuple() {
        let value = serde_json::to_value(NewsQueue::fresh_process()).expect("serializable");
        assert_eq!(
            value,
            serde_json::json!({"vehicle_income_warn": true, "items": []})
        );
        let mut queue = NewsQueue::fresh_process();
        assert!(queue.insert(probe_item()).is_ok());
        let value = serde_json::to_value(&queue).expect("serializable");
        assert_eq!(
            value,
            serde_json::json!({"vehicle_income_warn": true, "items":
                [{"company": 0, "vehicle": 0, "string": 761, "profit": -1125}]})
        );
    }

    #[test]
    fn trimming_beyond_the_single_insert_refuses() {
        let mut queue = NewsQueue::fresh_process();
        for index in 0..MAX_NEWS_AMOUNT {
            assert!(
                queue
                    .insert(NewsItem {
                        company: 0,
                        vehicle: index as u32,
                        string: 0x2F9,
                        profit: -1125,
                    })
                    .is_ok()
            );
        }
        assert_eq!(queue.len(), MAX_NEWS_AMOUNT);
        let error = queue
            .insert(probe_item())
            .expect_err("trimming must refuse");
        assert!(
            error
                .to_string()
                .contains("vehicle_year: news trimming beyond the single insert"),
            "{error}"
        );
    }
}
