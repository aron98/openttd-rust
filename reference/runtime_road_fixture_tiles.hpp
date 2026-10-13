// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RUNTIME_ROAD_FIXTURE_TILES_HPP
#define OTTD_REFERENCE_RUNTIME_ROAD_FIXTURE_TILES_HPP
#include "../station_cmd.h"
#include "../waypoint_cmd.h"
#include "../rail_cmd.h"
#include "../tunnelbridge_cmd.h"
#include "../newgrf_roadstop.h"
#include "../clear_map.h"
#include "../landscape.h"
namespace ReferenceRuntimeRoadFixture {
inline void RequireSuccess(const CommandCost &cost)
{
    if (cost.Failed()) throw std::runtime_error("Native road coverage infrastructure command failed");
}
inline std::array<TileIndex, 7> BuildCoverageTiles()
{
    if (Map::SizeX() != 64 || Map::SizeY() != 64) throw std::runtime_error("Road coverage fixture requires clear 64-square map");
    const TileIndex road = TileXY(8, 4), crossing = TileXY(8, 6), bus = TileXY(8, 8), truck = TileXY(12, 8);
    const TileIndex waypoint = TileXY(8, 12), bridge = TileXY(8, 16), tunnel = TileXY(27, 12);
    for (TileIndex tile : {road, crossing, bus, truck, waypoint, bridge, TileXY(12, 16)}) {
        if (!IsTileType(tile, MP_CLEAR) || TileHeight(tile) != 4) throw std::runtime_error("Coverage pad is not clear flat baseline");
    }
    RequireSuccess(Command<CMD_BUILD_ROAD>::Do(DoCommandFlag::Execute, road, ROAD_X, ROADTYPE_ROAD, DRD_NONE, TownID::Invalid()));
    RequireSuccess(Command<CMD_BUILD_SINGLE_RAIL>::Do(DoCommandFlag::Execute, crossing, RAILTYPE_RAIL, TRACK_Y, false));
    RequireSuccess(Command<CMD_BUILD_ROAD>::Do(DoCommandFlag::Execute, crossing, ROAD_X, ROADTYPE_ROAD, DRD_NONE, TownID::Invalid()));
    RequireSuccess(Command<CMD_BUILD_ROAD_STOP>::Do(DoCommandFlag::Execute, bus, 1, 1, RoadStopType::Bus, true, DIAGDIR_NE, ROADTYPE_ROAD, ROADSTOP_CLASS_DFLT, 0, NEW_STATION, true));
    RequireSuccess(Command<CMD_BUILD_ROAD_STOP>::Do(DoCommandFlag::Execute, truck, 1, 1, RoadStopType::Truck, true, DIAGDIR_NE, ROADTYPE_ROAD, ROADSTOP_CLASS_DFLT, 0, NEW_STATION, true));
    RequireSuccess(Command<CMD_BUILD_ROAD>::Do(DoCommandFlag::Execute, waypoint, ROAD_X, ROADTYPE_ROAD, DRD_NONE, TownID::Invalid()));
    RequireSuccess(Command<CMD_BUILD_ROAD_WAYPOINT>::Do(DoCommandFlag::Execute, waypoint, AXIS_X, 1, 1, ROADSTOP_CLASS_WAYP, 0, NEW_STATION, true));
    RequireSuccess(Command<CMD_BUILD_BRIDGE>::Do(DoCommandFlag::Execute, TileXY(12, 16), bridge, TRANSPORT_ROAD, BridgeType(0), uint8_t(ROADTYPE_ROAD)));
    for (uint y = 4; y <= 20; ++y) for (uint x = 24; x <= 40; ++x) {
        TileIndex tile = TileXY(x, y);
        if (!IsTileType(tile, MP_CLEAR) || TileHeight(tile) != 4) throw std::runtime_error("Road tunnel mound overlaps infrastructure");
        SetTileHeight(tile, 4 + std::min({x - 24, y - 4, 40 - x, 20 - y, 4U}));
    }
    RequireSuccess(Command<CMD_BUILD_TUNNEL>::Do(DoCommandFlag::Execute, tunnel, TRANSPORT_ROAD, uint8_t(ROADTYPE_ROAD)));
    return {road, crossing, bus, truck, waypoint, bridge, tunnel};
}
inline void SparseWitness(TileIndex depot, EngineID engine)
{
    std::vector<VehicleID> extras;
    do {
        auto [cost, id, capacity, mail_capacity, capacities] = Command<CMD_BUILD_VEHICLE>::Do(DoCommandFlag::Execute, depot, engine, false, INVALID_CARGO, INVALID_CLIENT_ID);
        RequireSuccess(cost);
        if (!Vehicle::IsValidID(id)) throw std::runtime_error("Missing native sparse fixture vehicle");
        extras.push_back(id);
    } while (extras.back().base() < 128);
    const VehicleID high = extras.back();
    for (VehicleID id : extras) {
        if (id == high) continue;
        RequireSuccess(Command<CMD_SELL_VEHICLE>::Do(DoCommandFlag::Execute, id, false, false, INVALID_CLIENT_ID));
    }
}
inline void PlaceWitnesses(const std::array<TileIndex, 7> &tiles)
{
    size_t index = 0;
    for (Vehicle *vehicle : Vehicle::Iterate()) {
        if (index == tiles.size()) break;
        RoadVehicle *v = RoadVehicle::From(vehicle);
        v->tile = tiles[index++];
        v->x_pos = TileX(v->tile) * TILE_SIZE + TILE_SIZE / 2;
        v->y_pos = TileY(v->tile) * TILE_SIZE + TILE_SIZE / 2;
        v->z_pos = GetSlopePixelZ(v->x_pos, v->y_pos, true);
        v->direction = DIR_NE;
        v->state = TRACKDIR_X_NE;
        v->vehstatus.Reset(VehState::Hidden);
        v->UpdatePosition();
    }
    if (index != tiles.size()) throw std::runtime_error("Missing tile-category witness vehicles");
}
}
#endif
