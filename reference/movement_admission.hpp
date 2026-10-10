// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_ADMISSION_HPP
#define OTTD_REFERENCE_MOVEMENT_ADMISSION_HPP
#include "../road_map.h"
#include "../newgrf_config.h"
#include "../game/game.hpp"
#include "../animated_tile_func.h"
#include "../progress.h"
#include "../economy_base.h"
#include "../linkgraph/linkgraph.h"
#include "reference_movement_forbidden.hpp"
namespace ReferenceMovement {
inline void ModeGuard()
{
    using ReferenceWorld::Require;
    const char *enabled = std::getenv("OTTD_MOVEMENT_OBSERVE");
    Require(enabled != nullptr && std::string_view(enabled) == "1", "invalid movement mode");
    for (const char *name : forbidden_modes) Require(std::getenv(name) == nullptr, "movement modes are mutually exclusive");
    Require(std::getenv("OTTD_REPLAY_PATH") != nullptr && std::getenv("OTTD_REPLAY_OUTPUT") != nullptr, "movement replay paths required");
    if (const char *prepare = std::getenv("OTTD_MOVEMENT_PREPARE")) Require(std::string_view(prepare) == "1", "invalid movement preparer mode");
    Require(!_networking && _game_mode == GM_NORMAL && !_pause_mode.Any() && !HasModalProgress(), "movement requires offline unpaused normal game");
}
inline void CommonWorldGuard()
{
    using ReferenceWorld::Require;
    Require(Map::SizeX() == 64 && Map::SizeY() == 64, "movement pilot requires 64-square map");
    Require(_grfconfig.empty() && _settings_game.game_creation.landscape == LandscapeType::Temperate, "movement pilot requires vanilla temperate content");
    Require(Game::GetInstance() == nullptr, "movement pilot excludes GameScript");
    Require(BaseStation::GetNumItems() == 0 && Industry::GetNumItems() == 0 && Object::GetNumItems() == 0, "movement pilot excludes populated station/industry/object domains");
    Require(CargoPacket::GetNumItems() == 0 && CargoPayment::GetNumItems() == 0, "movement pilot excludes cargo packets/payments");
    Require(LinkGraphJob::GetNumItems() == 0 && LinkGraph::GetNumItems() == 0, "movement pilot excludes linkgraphs");
    for (const Company *company : Company::Iterate()) Require(!company->is_ai && company->ai_instance == nullptr, "movement pilot excludes AI");
    for (const Town *town : Town::Iterate()) Require(town->road_build_months == 0 && town->fund_buildings_months == 0, "movement pilot excludes funded town programs");
    for (uint32_t id = 0; id < Map::Size(); ++id) {
        const TileIndex tile(id);
        if (IsTileType(tile, MP_CLEAR) || IsTileType(tile, MP_VOID)) continue;
        Require(IsTileType(tile, MP_ROAD) && (IsNormalRoad(tile) || IsRoadDepot(tile)), "movement pilot tile family");
        Require(GetTileSlope(tile) == SLOPE_FLAT && !IsBridgeAbove(tile), "movement pilot road must be flat");
        Require(GetRoadType(tile, RTT_ROAD) == ROADTYPE_ROAD && GetRoadType(tile, RTT_TRAM) == INVALID_ROADTYPE, "movement pilot vanilla road required");
        if (IsNormalRoad(tile)) Require(!HasRoadWorks(tile), "movement pilot roadworks unsupported");
    }
}
inline void WorldGuard()
{
    CommonWorldGuard();
    ReferenceWorld::Require(_settings_game.vehicle.roadveh_acceleration_model == AM_ORIGINAL, "movement pilot requires original acceleration");
}
inline void PrepareSeedGuard()
{
    ModeGuard();
    const char *prepare = std::getenv("OTTD_MOVEMENT_PREPARE");
    ReferenceWorld::Require(prepare != nullptr && std::string_view(prepare) == "1", "seed admission requires movement preparer");
    CommonWorldGuard();
    ReferenceWorld::Require(Vehicle::GetNumItems() == 0, "movement preparation requires empty vehicle pool");
    ReferenceWorld::Require(_settings_game.vehicle.roadveh_acceleration_model <= 1, "movement seed acceleration out of range");
}
inline const RoadVehicle *Subject(uint32_t id)
{
    using ReferenceWorld::Require;
    Require(Vehicle::GetNumItems() == 1, "movement pilot requires exactly one vehicle");
    const Vehicle *v = Vehicle::GetIfValid(VehicleID(id));
    Require(v != nullptr && v->type == VEH_ROAD, "movement pilot subject ID/family");
    const RoadVehicle *r = RoadVehicle::From(v);
    Require(r->IsFrontEngine() && r->Next() == nullptr, "movement pilot singlepart primary required");
    Require(r->GetGRF() == nullptr && r->roadtype == ROADTYPE_ROAD, "movement pilot custom vehicle content");
    Require(r->cargo.TotalCount() == 0 && r->GetNumOrders() == 0, "movement pilot cargo/order domain");
    Require(!r->vehstatus.Test(VehState::Crashed) && r->breakdown_ctr == 0 && r->breakdown_delay == 0, "movement pilot crash/breakdown reached");
    return r;
}
inline uint64_t Bounded(const Json &value, uint64_t maximum)
{
    ReferenceWorld::Require(value.is_number_unsigned() || (value.is_number_integer() && value.get<int64_t>() >= 0), "movement nonnegative integer required");
    const uint64_t result = value.get<uint64_t>();
    ReferenceWorld::Require(result <= maximum, "movement host bound exceeds limit");
    return result;
}
}
#endif
