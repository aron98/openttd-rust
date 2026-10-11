// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_HOUSE_OBSERVER_HPP
#define OTTD_REFERENCE_HOUSE_OBSERVER_HPP
#include "house.h"
#include "town.h"
#include "newgrf_house.h"
#include "core/random_func.hpp"
#include "timer/timer_game_tick.h"
#include "script/api/script_object.hpp"
#include "3rdparty/nlohmann/json.hpp"
#include <fstream>
extern TileIndex _cur_tileloop_tile;
extern uint8_t _trees_tick_ctr;
std::span<const uint> ReferenceHouseGlobalClassCounts();
namespace ReferenceHouse {
using Json = nlohmann::json;
inline uint64_t world_calls = 0;
inline Json Cache()
{
    Json towns = Json::array();
    for (const Town *town : Town::Iterate()) {
        const auto &cache = town->cache;
        towns.push_back({{"id", town->index.base()}, {"population", cache.population},
            {"num_houses", cache.num_houses}, {"zone_radii", cache.squared_town_zone_radius},
            {"id_count", cache.building_counts.id_count}, {"class_count", cache.building_counts.class_count}});
    }
    Json counts = Json::array();
    for (uint count : GetBuildingHouseIDCounts()) counts.push_back(count);
    Json classes = Json::array();
    for (uint count : ReferenceHouseGlobalClassCounts()) classes.push_back(count);
    return {{"towns", towns}, {"global_id_count", counts}, {"global_class_count", classes}};
}
inline Json Specs()
{
    Json specs = Json::array();
    for (const auto &spec : HouseSpec::Specs()) {
        specs.push_back({{"id", spec.Index()}, {"population", spec.population}, {"mail_generation", spec.mail_generation},
            {"building_flags", spec.building_flags.base()}, {"availability", spec.building_availability.base()},
            {"enabled", spec.enabled}, {"class_id", spec.class_id}, {"minimum_life", spec.minimum_life},
            {"min_year", spec.min_year.base()}, {"max_year", spec.max_year.base()},
            {"accepts_cargo", spec.accepts_cargo}, {"cargo_acceptance", spec.cargo_acceptance},
            {"callback_mask", spec.callback_mask.base()}});
    }
    return specs;
}
inline void Emit(const char *edge, uint32_t index)
{
    const char *path = std::getenv("OTTD_HOUSE_EVENTS");
    if (path == nullptr) return;
    static std::ofstream stream(path, std::ios::out | std::ios::app);
    static uint64_t sequence = 0;
    if (++sequence > 1000000) throw std::runtime_error("house observation event bound");
    Tile tile{TileIndex(index)};
    Json event = {{"sequence", sequence}, {"call", world_calls}, {"edge", edge}, {"tile", index},
        {"tick", TimerGameTick::counter}, {"cursor", _cur_tileloop_tile.base()},
        {"random", {_random.state[0], _random.state[1]}},
        {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"raw", {tile.type(), tile.height(), tile.m1(), tile.m2(), tile.m3(), tile.m4(), tile.m5(), tile.m6(), tile.m7(), tile.m8()}}};
    if (IsTileType(tile, MP_HOUSE)) {
        const Town *town = Town::GetByTile(tile);
        const auto flags = HouseSpec::Get(GetHouseType(tile))->building_flags;
        Json footprint = Json::array();
        auto part = [&](uint32_t offset) {
            Tile target{TileIndex(index + offset)};
            footprint.push_back({{"tile", index + offset}, {"raw", {target.type(), target.height(), target.m1(), target.m2(), target.m3(), target.m4(), target.m5(), target.m6(), target.m7(), target.m8()}}});
        };
        part(0);
        if (flags.Any(BUILDING_2_TILES_Y)) part(Map::SizeX());
        if (flags.Any(BUILDING_2_TILES_X)) part(1);
        if (flags.Any(BUILDING_HAS_4_TILES)) part(Map::SizeX() + 1);
        event["footprint"] = footprint;
        Json supplied = Json::array();
        for (const auto &cargo : town->supplied) supplied.push_back({{"cargo", cargo.cargo},
            {"production", cargo.history[THIS_MONTH].production}, {"transported", cargo.history[THIS_MONTH].transported}});
        event["town"] = town->index.base();
        event["population"] = town->cache.population;
        event["supplied"] = supplied;
    }
    stream << event.dump() << '\n';
    if (!stream.good()) throw std::runtime_error("house observation write failed");
}
class Scope {
    uint32_t tile;
public:
    explicit Scope(uint32_t tile) : tile(tile) { Emit("enter", tile); }
    ~Scope() noexcept(false) { Emit("leave", tile); }
    Scope(const Scope &) = delete;
    Scope &operator=(const Scope &) = delete;
};
inline void EmitWorld(const char *edge)
{
    const char *path = std::getenv("OTTD_HOUSE_WORLD_EVENTS");
    if (path == nullptr) return;
    static std::ofstream stream(path, std::ios::out | std::ios::app);
    static uint64_t sequence = 0;
    if (++sequence > 1000000) throw std::runtime_error("house world event bound");
    const auto &script = ScriptObject::GetRandomizer(OWNER_NONE);
    Json event = {{"sequence", sequence}, {"call", world_calls}, {"edge", edge},
        {"tick", TimerGameTick::counter}, {"cursor", _cur_tileloop_tile.base()},
        {"random", {_random.state[0], _random.state[1]}},
        {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"script_none_random", {script.state[0], script.state[1]}}, {"trees_tick_counter", _trees_tick_ctr}};
    stream << event.dump() << '\n';
    if (!stream.good()) throw std::runtime_error("house world event write failed");
}
class WorldScope {
public:
    WorldScope() { ++world_calls; EmitWorld("enter"); }
    ~WorldScope() noexcept(false) { EmitWorld("leave"); }
    WorldScope(const WorldScope &) = delete;
    WorldScope &operator=(const WorldScope &) = delete;
};
}
#endif
