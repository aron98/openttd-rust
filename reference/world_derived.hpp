// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_WORLD_DERIVED_HPP
#define OTTD_REFERENCE_WORLD_DERIVED_HPP
#include "reference_world.hpp"
#include "reference_content.hpp"
#include "reference_terrain.hpp"
#include "reference_allocation.hpp"
#include "reference_runtime_road.hpp"
#include "reference_runtime_road_fixture.hpp"
#include "../station_base.h"
#include "../group.h"
#include "../industry.h"
#include "../town.h"
#include "../economy_base.h"
#include <cstdio>

namespace ReferenceWorld {
struct CargoAccess {
    template <class Instance, class Container>
    static uint64_t Periods(const CargoList<Instance, Container> &cargo)
    {
        return cargo.cargo_periods_in_transit;
    }
    static void CorruptPeriods(VehicleCargoList &cargo) { cargo.cargo_periods_in_transit ^= 1; }
};
template <typename T> inline Json Id(const T *object)
{
    return object == nullptr ? Json(nullptr) : Json(object->index.base());
}
inline Json PacketIds(const auto &packets)
{
    Json ids = Json::array();
    for (const CargoPacket *packet : packets) ids.push_back(packet->index.base());
    return ids;
}
inline Json PacketGroup(const auto &packets, Json owner, Json cargo_type, Json next_hop)
{
    Json result = {{"owner", std::move(owner)}, {"cargo_type", std::move(cargo_type)},
        {"next_hop", std::move(next_hop)}, {"packets", PacketIds(packets)},
        {"count", uint64_t{0}}, {"periods_in_transit", uint64_t{0}}, {"feeder_share", int64_t{0}}};
    uint32_t count = 0;
    uint64_t periods = 0;
    Money feeder = 0;
    for (const CargoPacket *packet : packets) {
        count += packet->Count();
        periods += uint64_t{packet->Count()} * packet->GetPeriodsInTransit();
        feeder += packet->GetFeederShare();
    }
    result["count"] = count;
    result["periods_in_transit"] = periods;
    result["feeder_share"] = static_cast<int64_t>(feeder);
    return result;
}
inline void AfterLoad()
{
    ReferenceRuntimeRoadFixture::Prepare();
    ReferenceContent::Observe();
    ReferenceTerrain::Observe();
    ReferenceAllocation::Observe();
    ReferenceRuntimeRoad::Observe();
    const char *path = std::getenv("OTTD_WORLD_DERIVED_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (const char *control = std::getenv("OTTD_WORLD_CORRUPT_DERIVED")) {
        bool changed = false;
        if (std::string_view(control) == "group-children") {
            for (Group *group : Group::Iterate()) {
                if (group->children.empty()) continue;
                group->children.clear();
                changed = true;
                break;
            }
        } else if (std::string_view(control) == "cargo-cache") {
            for (Vehicle *vehicle : Vehicle::Iterate()) {
                if (!IsCompanyBuildableVehicleType(vehicle) || vehicle->cargo.TotalCount() == 0) continue;
                CargoAccess::CorruptPeriods(vehicle->cargo);
                changed = true;
                break;
            }
        } else if (std::string_view(control) == "cargo-payment") {
            for (Vehicle *vehicle : Vehicle::Iterate()) {
                if (vehicle->cargo_payment == nullptr) continue;
                vehicle->cargo_payment = nullptr;
                changed = true;
                break;
            }
        }
        Require(changed, "Derived control did not change a native cache");
        std::fprintf(stderr, "WORLD_DERIVED_CONTROL %s\n", control);
    }
    Json result = {{"vehicles", Json::array()}, {"order_lists", Json::array()},
        {"cargo_lists", Json::array()}, {"groups", Json::array()},
        {"road_stop_chains", Json::array()}, {"storage_owners", Json::array()},
        {"cargo_payments", Json::array()}};
    for (const Vehicle *vehicle : Vehicle::Iterate()) {
        result["vehicles"].push_back({{"id", vehicle->index.base()},
            {"previous", Id(vehicle->Previous())}, {"first", Id(vehicle->First())},
            {"previous_shared", Id(vehicle->PreviousShared())}});
        if (IsCompanyBuildableVehicleType(vehicle)) {
            result["cargo_lists"].push_back({{"owner", {{"pool", "VEHS"}, {"id", vehicle->index.base()}}},
                {"cargo_type", nullptr}, {"next_hop", nullptr}, {"packets", PacketIds(*vehicle->cargo.Packets())},
                {"count", vehicle->cargo.TotalCount()}, {"periods_in_transit", CargoAccess::Periods(vehicle->cargo)},
                {"feeder_share", static_cast<int64_t>(vehicle->cargo.GetFeederShare())}});
        }
        if (const CargoPayment *payment = vehicle->cargo_payment) {
            Require(payment->front == vehicle && payment->current_station == vehicle->last_station_visited, "Inconsistent native payment cache");
            result["cargo_payments"].push_back({{"id", payment->index.base()}, {"vehicle", vehicle->index.base()}});
        }
    }
    for (const OrderList *orders : OrderList::Iterate()) {
        Json vehicles = Json::array();
        for (const Vehicle *vehicle = orders->GetFirstSharedVehicle(); vehicle != nullptr; vehicle = vehicle->NextShared()) {
            Require(vehicles.size() < Vehicle::GetPoolSize(), "Cyclic native shared orders");
            vehicles.push_back(vehicle->index.base());
        }
        Require(vehicles.size() == orders->GetNumVehicles(), "Native order vehicle cache differs");
        result["order_lists"].push_back({{"id", orders->index.base()},
            {"first_shared", Id(orders->GetFirstSharedVehicle())}, {"vehicles", std::move(vehicles)},
            {"num_manual_orders", orders->GetNumManualOrders()},
            {"total_duration", orders->GetTotalDuration()},
            {"timetable_duration", orders->GetTimetableDurationIncomplete()}});
    }
    for (const Station *station : Station::Iterate()) {
        for (size_t type = 0; type < station->goods.size(); ++type) {
            const GoodsEntry &goods = station->goods[type];
            if (!goods.HasData()) continue;
            const auto &cargo = goods.GetData().cargo;
            uint32_t count = 0;
            uint64_t periods = 0;
            for (auto it = cargo.Packets()->begin(); it != cargo.Packets()->end(); ++it) {
                Json entry = PacketGroup(it->second, {{"pool", "STNN"}, {"id", station->index.base()}}, type, it->first.base());
                count += entry["count"].get<uint32_t>();
                periods += entry["periods_in_transit"].get<uint64_t>();
                result["cargo_lists"].push_back(std::move(entry));
            }
            Require(count == cargo.TotalCount() && periods == CargoAccess::Periods(cargo), "Native station cargo cache differs");
        }
        for (bool bus : {true, false}) {
            Json stops = Json::array();
            for (const RoadStop *stop = bus ? station->bus_stops : station->truck_stops; stop != nullptr; stop = stop->next) {
                Require(stops.size() < RoadStop::GetPoolSize(), "Cyclic native road stops");
                stops.push_back(stop->index.base());
            }
            result["road_stop_chains"].push_back({{"station", station->index.base()}, {"kind", bus ? "bus" : "truck"}, {"stops", std::move(stops)}});
        }
        if (station->airport.psa != nullptr) result["storage_owners"].push_back({{"id", station->airport.psa->index.base()}, {"owner", {{"pool", "STNN"}, {"id", station->index.base()}}}});
    }
    for (const Group *group : Group::Iterate()) {
        Json children = Json::array();
        for (GroupID child : group->children) children.push_back(child.base());
        result["groups"].push_back({{"id", group->index.base()}, {"children", std::move(children)}});
    }
    for (const Town *town : Town::Iterate()) {
        for (const PersistentStorage *storage : town->psa_list) result["storage_owners"].push_back({{"id", storage->index.base()}, {"owner", {{"pool", "CITY"}, {"id", town->index.base()}}}});
    }
    for (const Industry *industry : Industry::Iterate()) {
        if (industry->psa != nullptr) result["storage_owners"].push_back({{"id", industry->psa->index.base()}, {"owner", {{"pool", "INDY"}, {"id", industry->index.base()}}}});
    }
    std::sort(result["storage_owners"].begin(), result["storage_owners"].end(), [](const Json &a, const Json &b) { return a["id"] < b["id"]; });
    std::sort(result["cargo_payments"].begin(), result["cargo_payments"].end(), [](const Json &a, const Json &b) { return a["id"] < b["id"]; });
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << result.dump() << '\n';
    stream.close();
}
}
#endif
