// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_INDUSTRY_OBSERVER_HPP
#define OTTD_REFERENCE_INDUSTRY_OBSERVER_HPP
#include "industry.h"
#include "town.h"
#include "economy_base.h"
#include "core/random_func.hpp"
#include "timer/timer_game_tick.h"
#include "3rdparty/nlohmann/json.hpp"
#include <fstream>
extern TileIndex _cur_tileloop_tile;
namespace ReferenceIndustry {
using Json = nlohmann::json;
inline Json IndustryState(const Industry *industry)
{
    if (industry == nullptr) return nullptr;
    Json produced = Json::array();
    for (const auto &cargo : industry->produced) {
        produced.push_back({{"cargo", cargo.cargo}, {"waiting", cargo.waiting}, {"rate", cargo.rate},
            {"production", cargo.history[0].production}, {"transported", cargo.history[0].transported}});
    }
    Json accepted = Json::array();
    for (const auto &cargo : industry->accepted) {
        accepted.push_back({{"cargo", cargo.cargo}, {"waiting", cargo.waiting}, {"accumulated_waiting", cargo.accumulated_waiting}});
    }
    Json stations = Json::array();
    for (const Station *station : industry->stations_near) stations.push_back(station->index.base());
    return {{"id", industry->index.base()}, {"type", industry->type}, {"counter", industry->counter},
        {"prod_level", industry->prod_level}, {"produced", produced}, {"accepted", accepted},
        {"location", {industry->location.tile.base(), industry->location.w, industry->location.h}},
        {"town", industry->town == nullptr ? Json(nullptr) : Json(industry->town->index.base())},
        {"neutral_station", industry->neutral_station == nullptr ? Json(nullptr) : Json(industry->neutral_station->index.base())},
        {"stations_near", stations}, {"part_of_subsidy", industry->part_of_subsidy.base()},
        {"exclusive_consumer", industry->exclusive_consumer.base()}};
}
inline Json Builder()
{
    Json rows = Json::array();
    for (const auto &row : _industry_builder.builddata) {
        rows.push_back({{"probability", row.probability}, {"min_number", row.min_number},
            {"target_count", row.target_count}, {"max_wait", row.max_wait}, {"wait_count", row.wait_count}});
    }
    return {{"wanted_inds", _industry_builder.wanted_inds}, {"types", rows}};
}
inline Json Cache()
{
    Json industries = Json::array();
    for (const Industry *industry : Industry::Iterate()) industries.push_back(IndustryState(industry));
    Json types = Json::array();
    for (const auto &members : Industry::industries) {
        Json ids = Json::array();
        for (IndustryID id : members) ids.push_back(id.base());
        types.push_back(ids);
    }
    return {{"industries", industries}, {"type_members", types}, {"builder", Builder()},
        {"daily_increment", _economy.industry_daily_increment}};
}
inline void Emit(const char *phase, const char *edge, const Industry *industry, TileIndex tile)
{
    const char *path = std::getenv("OTTD_INDUSTRY_EVENTS");
    if (path == nullptr) return;
    static std::ofstream stream(path, std::ios::out | std::ios::app);
    static uint64_t sequence = 0;
    if (++sequence > 400000) throw std::runtime_error("industry observation event bound");
    Json event = {{"sequence", sequence}, {"phase", phase}, {"edge", edge},
        {"tick", TimerGameTick::counter}, {"cursor", _cur_tileloop_tile.base()},
        {"random", {_random.state[0], _random.state[1]}},
        {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"daily_counter", _economy.industry_daily_change_counter}, {"industry", IndustryState(industry)}};
    if (tile != INVALID_TILE) {
        Tile value{tile};
        event["tile"] = tile.base();
        event["raw"] = {value.type(), value.height(), value.m1(), value.m2(), value.m3(), value.m4(), value.m5(), value.m6(), value.m7(), value.m8()};
    }
    if (std::string_view(phase) == "daily" || std::string_view(phase) == "builder") event["builder"] = Builder();
    stream << event.dump() << '\n';
    if (!stream) throw std::runtime_error("industry observation write failed");
}
struct Scope {
    const char *phase;
    IndustryID industry_id;
    TileIndex tile;
    Scope(const char *phase, const Industry *industry = nullptr, TileIndex tile = INVALID_TILE) : phase(phase), industry_id(industry == nullptr ? IndustryID::Invalid() : industry->index), tile(tile)
    { Emit(phase, "enter", industry, tile); }
    ~Scope() noexcept(false) { Emit(phase, "leave", Industry::GetIfValid(industry_id), tile); }
};
}
#endif
