// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ALLOCATION_HPP
#define OTTD_REFERENCE_ALLOCATION_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../company_base.h"
#include "../core/pool_func.hpp"
#include <fstream>

namespace ReferenceAllocation {
using Json = nlohmann::json;
template <uint16_t Max, size_t Growth> struct Item;
template <uint16_t Max> using ID = PoolID<uint16_t, struct AllocationTag, Max, UINT16_MAX>;
template <uint16_t Max, size_t Growth> using ScratchPool = Pool<Item<Max, Growth>, ID<Max>, Growth>;
template <uint16_t Max, size_t Growth> inline ScratchPool<Max, Growth> scratch("reference allocation");
template <uint16_t Max, size_t Growth> struct Item : ScratchPool<Max, Growth>::template PoolItem<&scratch<Max, Growth>> {};

template <uint16_t Max, size_t Growth> inline Json Snapshot()
{
    auto &pool = scratch<Max, Growth>;
    Json occupied = Json::array();
    for (size_t i = 0; i < pool.first_unused; ++i) if (pool.IsValidID(i)) occupied.push_back(i);
    return {{"first_free", pool.first_free}, {"first_unused", pool.first_unused}, {"items", pool.items}, {"slots", pool.data.size()}, {"occupied", occupied}};
}

template <uint16_t Max, size_t Growth> inline Json PoolTrace()
{
    auto &pool = scratch<Max, Growth>;
    using Object = Item<Max, Growth>;
    pool.CleanPool();
    Json trace = Json::array();
    auto step = [&](std::string_view action, uint16_t id = 0) {
        Json result = nullptr;
        if (action == "insert") result = (new (ID<Max>(id)) Object)->index.base();
        if (action == "allocate" && pool.CanAllocate()) result = (new Object)->index.base();
        if (action == "free") delete pool.Get(id);
        if (action == "reset") pool.CleanPool();
        if (action == "can_allocate") result = pool.CanAllocate(id);
        trace.push_back({{"action", action}, {"id", id}, {"result", result}, {"state", Snapshot<Max, Growth>()}});
    };
    step("insert", Max - 1);
    if constexpr (Max > 74) { step("insert", 73); step("insert", 64); }
    step("insert", 0);
    step("allocate");
    step("free", Max - 1);
    step("free", 0);
    step("allocate");
    while (pool.CanAllocate()) step("allocate");
    step("allocate");
    step("can_allocate", 1);
    step("free", 1);
    step("allocate");
    step("reset");
    uint32_t seed = 0x8b91723a;
    for (size_t i = 0; i < 512; ++i) {
        seed = seed * 1664525U + 1013904223U;
        const uint16_t id = seed % Max;
        if ((seed >> 16) % 3 == 0) {
            if (pool.IsValidID(id)) step("free", id); else step("insert", id);
        } else if ((seed >> 16) % 3 == 1) {
            step("allocate");
        } else {
            step("can_allocate", seed % (Max + 1));
        }
    }
    step("reset");
    pool.CleanPool();
    return {{"limit", Max}, {"growth", Growth}, {"trace", trace}};
}

inline Json UnitTrace()
{
    std::array<std::array<FreeUnitIDGenerator, 4>, 2> units;
    Json trace = Json::array();
    auto step = [&](uint8_t company, uint8_t type, std::string_view action, uint16_t id = 0) {
        auto &unit = units[company][type];
        Json result = nullptr;
        if (action == "use") result = unit.UseID(id);
        if (action == "release") unit.ReleaseID(id);
        if (action == "fill") for (uint32_t number = 1; number < UINT16_MAX; ++number) unit.UseID(number);
        trace.push_back({{"company", company}, {"type", type}, {"action", action}, {"id", id}, {"result", result}, {"next", unit.NextID()}});
    };
    for (auto id : {0, 65535, 1, 1, 64, 65}) step(0, 1, "use", id);
    step(0, 0, "next"); step(1, 1, "next"); step(0, 0, "use", 1);
    for (auto id : {0, 65535, 1, 2}) step(0, 1, "release", id);
    step(0, 1, "fill"); step(0, 1, "release", 65534);
    step(0, 0, "next"); step(1, 1, "next");
    return trace;
}

inline void Observe()
{
    const char *path = std::getenv("OTTD_ALLOCATION_PROBES_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(path).good()) throw std::runtime_error("Allocation observation already exists");
    Json result = {{"schema_version", 1}, {"pools", {PoolTrace<130, 8>(), PoolTrace<3, 1>()}}, {"units", UnitTrace()}};
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << result.dump() << '\n';
}
}
#endif
