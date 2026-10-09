// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_WORLD_DERIVED_HPP
#define OTTD_REFERENCE_WORLD_DERIVED_HPP
#include "reference_world.hpp"
#include "../station_base.h"
#include "../group.h"
#include "../industry.h"
#include "../town.h"
#include "../economy_base.h"

namespace ReferenceWorld {
struct CargoAccess {
    template <class Instance, class Container>
    static uint64_t Periods(const CargoList<Instance, Container> &cargo)
    {
        return cargo.cargo_periods_in_transit;
    }
};
template <typename T> inline Json Id(const T *object)
{
    return object == nullptr ? Json(nullptr) : Json(object->index.base());
}
inline Json PacketGroup(const auto &packets, Json owner, Json cargo_type, Json next_hop)
{
    Json result = {{"owner", std::move(owner)}, {"cargo_type", std::move(cargo_type)},
        {"next_hop", std::move(next_hop)}, {"packets", Json::array()},
        {"count", uint64_t{0}}, {"periods_in_transit", uint64_t{0}}, {"feeder_share", int64_t{0}}};
    uint64_t count = 0, periods = 0;
    int64_t feeder = 0;
    for (const CargoPacket *packet : packets) {
        result["packets"].push_back(packet->index.base());
        count += packet->Count();
        periods += uint64_t{packet->Count()} * packet->GetPeriodsInTransit();
        feeder += packet->GetFeederShare();
    }
    result["count"] = count;
    result["periods_in_transit"] = periods;
    result["feeder_share"] = feeder;
    return result;
}
inline void AfterLoad()
{
    const char *path = std::getenv("OTTD_WORLD_DERIVED_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    Json result = {{"vehicles", Json::array()}, {"order_lists", Json::array()},
        {"cargo_lists", Json::array()}, {"groups", Json::array()},
        {"road_stop_chains", Json::array()}, {"storage_owners", Json::array()},
        {"cargo_payments", Json::array()}};
    for (const Vehicle *vehicle : Vehicle::Iterate()) {
        result["vehicles"].push_back({{"id", vehicle->index.base()},
            {"previous", Id(vehicle->Previous())}, {"first", Id(vehicle->First())},
            {"previous_shared", Id(vehicle->PreviousShared())}});
        if (IsCompanyBuildableVehicleType(vehicle)) {
            Json cargo = PacketGroup(*vehicle->cargo.Packets(), {{"pool", "VEHS"}, {"id", vehicle->index.base()}}, nullptr, nullptr);
            cargo["count"] = vehicle->cargo.TotalCount();
            cargo["periods_in_transit"] = CargoAccess::Periods(vehicle->cargo);
            cargo["feeder_share"] = static_cast<int64_t>(vehicle->cargo.GetFeederShare());
            result["cargo_lists"].push_back(std::move(cargo));
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
            uint64_t count = 0, periods = 0;
            for (auto it = cargo.Packets()->begin(); it != cargo.Packets()->end(); ++it) {
                Json entry = PacketGroup(it->second, {{"pool", "STNN"}, {"id", station->index.base()}}, type, it->first.base());
                count += entry["count"].get<uint64_t>();
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
        for (const Group *child : Group::Iterate()) if (child->parent == group->index) children.push_back(child->index.base());
        result["groups"].push_back({{"id", group->index.base()}, {"children", std::move(children)}});
    }
    for (const Town *town : Town::Iterate()) {
        for (const PersistentStorage *storage : town->psa_list) result["storage_owners"].push_back({{"id", storage->index.base()}, {"owner", {{"pool", "CITY"}, {"id", town->index.base()}}}});
    }
    for (const Industry *industry : Industry::Iterate()) {
        if (industry->psa != nullptr) result["storage_owners"].push_back({{"id", industry->psa->index.base()}, {"owner", {{"pool", "INDY"}, {"id", industry->index.base()}}}});
    }
    std::sort(result["storage_owners"].begin(), result["storage_owners"].end(), [](const Json &a, const Json &b) { return a["id"] < b["id"]; });
    for (const CargoPayment *payment : CargoPayment::Iterate()) result["cargo_payments"].push_back({{"id", payment->index.base()}, {"vehicle", Id(payment->front)}});
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << result.dump() << '\n';
    stream.close();
}
}
#endif
