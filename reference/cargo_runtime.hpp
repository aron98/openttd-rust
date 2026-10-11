// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CARGO_RUNTIME_HPP
#define OTTD_REFERENCE_CARGO_RUNTIME_HPP
#include "../cargopacket.h"
#include "../station_base.h"
#include "../town.h"
#include "../industry.h"

struct ReferenceCargoRuntimeAccess {
    template <class T, class C> static uint64_t Periods(const CargoList<T, C> &list) { return list.cargo_periods_in_transit; }
};
namespace ReferenceCargoRuntime {
using Json = nlohmann::json;
inline Json Snapshot()
{
    Json vehicles = Json::object(), stations = Json::object(), town_near = Json::object(), industry_near = Json::object();
    for (const Vehicle *v : Vehicle::Iterate()) {
        Json packets = Json::array(), actions = Json::array();
        for (const CargoPacket *cp : *v->cargo.Packets()) packets.push_back(cp->index.base());
        for (uint i = 0; i < VehicleCargoList::NUM_MOVE_TO_ACTION; ++i) actions.push_back(v->cargo.ActionCount(static_cast<VehicleCargoList::MoveToAction>(i)));
        vehicles[std::to_string(v->index.base())] = {{"packets", packets}, {"count", v->cargo.TotalCount()},
            {"periods", ReferenceCargoRuntimeAccess::Periods(v->cargo)}, {"feeder_share", static_cast<int64_t>(v->cargo.GetFeederShare())}, {"actions", actions}};
    }
    for (const Station *st : Station::Iterate()) {
        Json goods = Json::object();
        for (CargoType c = 0; c < NUM_CARGO; ++c) {
            const GoodsEntry &ge = st->goods[c];
            if (!ge.HasData()) continue;
            const StationCargoList &list = ge.GetData().cargo;
            Json packets = Json::object();
            for (const auto &[next, group] : *list.Packets()) {
                Json ids = Json::array();
                for (const CargoPacket *cp : group) ids.push_back(cp->index.base());
                packets[std::to_string(next.base())] = ids;
            }
            goods[std::to_string(c)] = {{"packets", packets}, {"count", list.AvailableCount()},
                {"periods", ReferenceCargoRuntimeAccess::Periods(list)}, {"reserved", list.ReservedCount()}};
        }
        stations[std::to_string(st->index.base())] = goods;
    }
    for (const Town *t : Town::Iterate()) {
        Json ids = Json::array(); for (const Station *st : t->stations_near) ids.push_back(st->index.base());
        town_near[std::to_string(t->index.base())] = ids;
    }
    for (const Industry *i : Industry::Iterate()) {
        Json ids = Json::array(); for (const Station *st : i->stations_near) ids.push_back(st->index.base());
        industry_near[std::to_string(i->index.base())] = ids;
    }
    return {{"pool", ReferenceOrderState::PoolState(_cargopacket_pool)}, {"vehicles", vehicles}, {"stations", stations},
        {"town_near", town_near}, {"industry_near", industry_near}};
}
}
#endif
