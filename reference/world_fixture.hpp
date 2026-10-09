// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_WORLD_FIXTURE_HPP
#define OTTD_REFERENCE_WORLD_FIXTURE_HPP

#include "reference_world.hpp"
#include "../economy_base.h"
#include "../economy_func.h"
#include "../industry.h"
#include "../newgrf_storage.h"
#include "../openttd.h"
#include "../station_base.h"
#include "../station_map.h"
#include "../town.h"
#include "../vehicle_base.h"
#include "../waypoint_base.h"
#include "../waypoint_cmd.h"
#include "../command_func.h"
#include "../core/backup_type.hpp"
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <string_view>

namespace ReferenceWorldFixture {
inline void Apply()
{
    const char *mode = std::getenv("OTTD_WORLD_FIXTURE_MODE");
    if (mode == nullptr || _game_mode == GM_MENU) return;
    static bool applied = false;
    if (applied) return;
    ReferenceWorld::Require(std::string_view(mode) == "storage-payment", "unknown world fixture mode");
    const char *manifest_path = std::getenv("OTTD_WORLD_FIXTURE_MANIFEST_PATH");
    ReferenceWorld::Require(manifest_path != nullptr, "fixture manifest path is required");
    ReferenceWorld::Require(Town::IsValidID(TownID(0)) && Industry::IsValidID(IndustryID(0)), "fixture requires town 0 and industry 0");
    ReferenceWorld::Require(Vehicle::IsValidID(VehicleID(21)), "fixture requires vehicle 21");
    Town *town = Town::Get(TownID(0));
    Industry *industry = Industry::Get(IndustryID(0));
    Vehicle *vehicle = Vehicle::Get(VehicleID(21));
    ReferenceWorld::Require(town->psa_list.empty() && industry->psa == nullptr, "fixture owners already have persistent storage");
    ReferenceWorld::Require(vehicle->type == VEH_ROAD && vehicle->IsPrimaryVehicle() && vehicle->First() == vehicle, "fixture vehicle must be a primary road vehicle");
    ReferenceWorld::Require(vehicle->cargo.TotalCount() > 0 && vehicle->cargo_payment == nullptr, "fixture requires loaded vehicle without cargo payment");
    ReferenceWorld::Require(IsTileType(vehicle->tile, MP_STATION), "fixture vehicle must be on a station tile");
    const StationID station_id = GetStationIndex(vehicle->tile);
    ReferenceWorld::Require(Station::IsValidID(station_id), "fixture vehicle station is invalid");
    Station *station = Station::Get(station_id);
    ReferenceWorld::Require(station->owner == vehicle->owner, "fixture station and vehicle ownership differs");
    for (const Vehicle *loading : station->loading_vehicles) {
        ReferenceWorld::Require(loading != vehicle, "fixture vehicle is already loading");
    }
    ReferenceWorld::Require(PersistentStorage::CanAllocateItem(2) && CargoPayment::CanAllocateItem(), "fixture pools are full");

    constexpr uint32_t authored_grfid = 0x54535552;
    PersistentStorage *town_storage = new PersistentStorage(authored_grfid, GSF_FAKE_TOWNS, town->xy);
    PersistentStorage *industry_storage = new PersistentStorage(authored_grfid, GSF_INDUSTRIES, industry->location.tile);
    town_storage->storage.at(0) = 12345;
    town_storage->storage.at(255) = -17;
    industry_storage->storage.at(0) = 54321;
    industry_storage->storage.at(255) = -29;
    town->psa_list.push_back(town_storage);
    industry->psa = industry_storage;

    const StationID previous_station = vehicle->last_station_visited;
    vehicle->last_station_visited = station_id;
    vehicle->current_order.MakeLoading(true);
    PrepareUnload(vehicle);
    CargoPayment *payment = vehicle->cargo_payment;
    ReferenceWorld::Require(payment != nullptr && payment->front == vehicle && payment->current_station == station_id, "native loading helper did not establish cargo payment");
    payment->route_profit = 12345;
    payment->visual_profit = 11234;
    payment->visual_transfer = 1111;

    Waypoint *buoy = nullptr;
    {
        AutoRestoreBackup company(_current_company, vehicle->owner);
        for (uint32_t index = 1; index < Map::Size(); ++index) {
            const TileIndex tile(index);
            if (!IsWaterTile(tile) || Command<CMD_BUILD_BUOY>::Do({}, tile).Failed()) continue;
            ReferenceWorld::Require(!Command<CMD_BUILD_BUOY>::Do({DoCommandFlag::Execute}, tile).Failed(), "native buoy command failed after successful test");
            buoy = Waypoint::GetByTile(tile);
            break;
        }
    }
    ReferenceWorld::Require(buoy != nullptr, "fixture map has no suitable buoy tile");
    _pause_mode.Set(PauseMode::Normal);

    ReferenceWorld::Json manifest = {
        {"schema_version", 1}, {"mode", mode},
        {"scope", "synthetic native serialization state; inert authored GRF identity; no callback or gameplay-command claim"},
        {"grfid", authored_grfid},
        {"paused", true},
        {"waypoint", {{"id", buoy->index.base()}, {"tile", buoy->xy.base()}, {"native_command", "CMD_BUILD_BUOY"}}},
        {"storage", {
            {{"id", town_storage->index.base()}, {"owner_pool", "CITY"}, {"owner_id", town->index.base()}, {"first", 12345}, {"last", -17}},
            {{"id", industry_storage->index.base()}, {"owner_pool", "INDY"}, {"owner_id", industry->index.base()}, {"first", 54321}, {"last", -29}}
        }},
        {"payment", {{"id", payment->index.base()}, {"vehicle", vehicle->index.base()},
            {"station", station_id.base()}, {"previous_station", previous_station.base()},
            {"route_profit", 12345}, {"visual_profit", 11234}, {"visual_transfer", 1111}}}
    };
    std::ofstream output(manifest_path, std::ios::out | std::ios::trunc);
    ReferenceWorld::Require(output.is_open(), "cannot open fixture manifest");
    output << manifest.dump() << '\n';
    output.close();
    ReferenceWorld::Require(!output.fail(), "cannot write fixture manifest");
    std::fprintf(stderr, "WORLD_FIXTURE storage-payment: %s\n", manifest.dump().c_str());
    applied = true;
}
}

#endif
