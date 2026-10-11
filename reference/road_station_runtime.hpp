// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ROAD_STATION_RUNTIME_HPP
#define OTTD_REFERENCE_ROAD_STATION_RUNTIME_HPP
#include "../station_base.h"
#include "../roadstop_base.h"
#include "../company_base.h"
#include "../core/pool_func.hpp"
#include "../core/random_func.hpp"

namespace ReferenceRoadStationRuntime {
using Json = nlohmann::json;
inline Json Area(const TileArea &area)
{
    if (area.tile == INVALID_TILE || area.w == 0 || area.h == 0) return nullptr;
    return {{"left", TileX(area.tile)}, {"top", TileY(area.tile)},
        {"right", TileX(area.tile) + area.w - 1}, {"bottom", TileY(area.tile) + area.h - 1}};
}
inline Json Snapshot()
{
    Json road = Json::object(), geometry = Json::object(), infrastructure = Json::object(), chains = Json::array();
    for (const Company *company : Company::Iterate()) {
        infrastructure[std::to_string(company->index.base())] = company->infrastructure.station;
        road[std::to_string(company->index.base())] = company->infrastructure.road;
    }
    for (const RoadStop *stop : RoadStop::Iterate()) chains.push_back({{"id", stop->index.base()}, {"xy", stop->xy.base()},
        {"status", stop->status.base()}, {"next", stop->next == nullptr ? Json(nullptr) : Json(stop->next->index.base())}});
    for (const Station *station : Station::Iterate()) {
        if ((station->facilities.base() & ~uint8_t{6}) != 0) continue;
        Json tiles = Json::array(), rect = nullptr;
        if (station->catchment_tiles.tile != INVALID_TILE) {
            BitmapTileIterator it(station->catchment_tiles);
            for (TileIndex tile = it; tile != INVALID_TILE; tile = ++it) tiles.push_back(tile.base());
        }
        if (!station->rect.IsEmpty()) rect = {{"left", station->rect.left}, {"top", station->rect.top},
            {"right", station->rect.right}, {"bottom", station->rect.bottom}};
        geometry[std::to_string(station->index.base())] = {{"station", rect},
            {"bus", Area(station->bus_station)}, {"truck", Area(station->truck_station)}, {"catchment", tiles}};
    }
    return {{"orders", ReferenceOrderState::Snapshot()}, {"depot_pool", ReferenceOrderState::PoolState(_depot_pool)},
        {"sale", ReferenceRuntimeRoad::SaleSnapshot()}, {"road_infrastructure", road}, {"station_pool", ReferenceOrderState::PoolState(_station_pool)},
        {"road_stop_pool", ReferenceOrderState::PoolState(_roadstop_pool)}, {"geometry", geometry},
        {"infrastructure", infrastructure}, {"stops", chains}, {"random", _random.state}};
}
}
#endif
