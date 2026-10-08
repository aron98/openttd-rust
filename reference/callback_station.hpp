// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_STATION_HPP
#define OTTD_REFERENCE_CALLBACK_STATION_HPP
#include "../station_base.h"
#include "reference_callback_timer.hpp"
namespace ReferenceCallbackStation {
using Json = nlohmann::json;
inline Json State()
{
    Json state = {{"random_state", {_random.state[0], _random.state[1]}}, {"stations", Json::array()}};
    for (Station *st : Station::Iterate()) {
        Json goods = Json::array();
        for (const GoodsEntry &ge : st->goods) {
            goods.push_back({{"status", ge.status.base()}, {"time_since_pickup", ge.time_since_pickup}, {"rating", ge.rating},
                {"last_speed", ge.last_speed}, {"last_age", ge.last_age}, {"amount_fract", ge.amount_fract}});
        }
        state["stations"].push_back({{"id", st->index.base()}, {"goods", goods}});
    }
    return state;
}
inline Json Run()
{
    _station_pool.CleanPool();
    Json cases = Json::array();
    Json empty = State();
    ReferenceCallbackTimer::Invoke(TimerGameEconomy::MONTH, TimerGameEconomy::Priority::STATION);
    cases.push_back({{"name", "station_month_empty"}, {"before", empty}, {"operation", {{"kind", "station_month"}}}, {"after", State()}});
    uint n = 0;
    for (uint16_t id : {0, 74, 4096, 63999}) {
        Station *st = new (StationID{id}) Station();
        for (GoodsEntry &ge : st->goods) {
            ge.status = GoodsEntry::States{uint8_t(n)};
            ge.time_since_pickup = uint8_t(n * 3); ge.rating = uint8_t(n * 5);
            ge.last_speed = uint8_t(n * 7); ge.last_age = uint8_t(n * 11); ge.amount_fract = uint8_t(n * 13);
            ++n;
        }
    }
    for (const char *name : {"station_month_all_status_bits", "station_month_repeat"}) {
        Json before = State();
        ReferenceCallbackTimer::Invoke(TimerGameEconomy::MONTH, TimerGameEconomy::Priority::STATION);
        cases.push_back({{"name", name}, {"before", before}, {"operation", {{"kind", "station_month"}}}, {"after", State()}});
    }
    return cases;
}
}
#endif
