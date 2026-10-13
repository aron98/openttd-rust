// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ORDER_STATE_HPP
#define OTTD_REFERENCE_ORDER_STATE_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../order_backup.h"
#include "../order_base.h"
#include "../vehicle_base.h"
#include "../station_base.h"
#include "../network/network.h"
#include "../network/network_func.h"
#include "../network/network_base.h"
#include "../timer/timer_game_tick.h"
#include "../core/random_func.hpp"
#include <filesystem>
#include <fstream>
#include <cstdio>

#ifdef DEBUG_DUMP_COMMANDS
#error Order lifecycle reference requires ordinary afterload backup deletion
#endif

struct ReferenceOrderAccess {
    using Json = nlohmann::json;
    static Json OrderFields(const Order &o)
    {
        return {{"type", o.type}, {"flags", o.flags}, {"dest", o.dest.base()},
            {"refit_cargo", o.refit_cargo}, {"wait_time", o.wait_time},
            {"travel_time", o.travel_time}, {"max_speed", o.max_speed}};
    }
    static Json Consist(const BaseConsist &v)
    {
        return {{"name", v.name}, {"current_order_time", v.current_order_time},
            {"current_order_time_bits", static_cast<uint32_t>(v.current_order_time)},
            {"lateness_counter", v.lateness_counter}, {"timetable_start", v.timetable_start},
            {"service_interval", v.service_interval}, {"cur_real_order_index", v.cur_real_order_index},
            {"cur_implicit_order_index", v.cur_implicit_order_index}, {"vehicle_flags", v.vehicle_flags.base()}};
    }
    static Json Backup(const OrderBackup &v, size_t pool_slot)
    {
        Json orders = Json::array();
        for (const Order &order : v.orders) orders.push_back(OrderFields(order));
        return {{"id", v.index.base()}, {"pool_slot", pool_slot}, {"user", v.user}, {"tile", v.tile.base()},
            {"group", v.group.base()}, {"clone", v.clone == nullptr ? Json(nullptr) : Json(v.clone->index.base())},
            {"orders", orders}, {"consist", Consist(v)}};
    }
};

namespace ReferenceOrderState {
using Json = nlohmann::json;
inline bool afterload_ready = false;
[[noreturn]] inline void HostError(const char *message)
{
    fmt::print(stderr, "Order reference host error: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
inline void Require(bool condition, const char *message) { if (!condition) HostError(message); }
inline void Write(const std::filesystem::path &path, const Json &value)
{
    Require(!std::filesystem::exists(path), "output already exists");
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << value.dump() << '\n';
    stream.close();
}
template <typename Pool> inline Json PoolState(const Pool &pool)
{
    Json occupied = Json::array();
    for (size_t id = 0; id < pool.first_unused; ++id) if (pool.data[id] != nullptr) occupied.push_back(id);
    return {{"first_free", pool.first_free}, {"first_unused", pool.first_unused},
        {"items", pool.items}, {"slots", pool.data.size()}, {"occupied", occupied}};
}
inline Json Role()
{
    Json clients = Json::array();
    for (const NetworkClientInfo *client : NetworkClientInfo::Iterate()) clients.push_back({
        {"id", static_cast<uint32_t>(client->client_id)}, {"name", client->client_name}, {"company", client->client_playas.base()}});
    return {{"networking", _networking}, {"server", _network_server},
        {"dedicated", _network_dedicated}, {"own_client_id", static_cast<uint32_t>(_network_own_client_id)}, {"clients", clients}};
}
inline Json Snapshot()
{
    Json lists = Json::array(), vehicles = Json::array(), backups = Json::array();
    for (const OrderList *list : OrderList::Iterate()) {
        Json members = Json::array(), orders = Json::array();
        for (const Vehicle *v = list->GetFirstSharedVehicle(); v != nullptr; v = v->NextShared()) {
            Require(members.size() <= Vehicle::GetNumItems(), "cyclic native order membership");
            members.push_back(v->index.base());
        }
        for (const Order &o : list->GetOrders()) orders.push_back(ReferenceOrderAccess::OrderFields(o));
        const Vehicle *head = list->GetFirstSharedVehicle();
        lists.push_back({{"id", list->index.base()}, {"first_shared", head == nullptr ? Json(nullptr) : Json(head->index.base())},
            {"num_vehicles", list->GetNumVehicles()}, {"num_manual_orders", list->GetNumManualOrders()},
            {"total_duration", list->GetTotalDuration()}, {"timetable_duration", list->GetTimetableDurationIncomplete()},
            {"members", members}, {"orders", orders}});
    }
    for (const Vehicle *v : Vehicle::Iterate()) vehicles.push_back({{"id", v->index.base()}, {"type", v->type},
        {"orders", v->orders == nullptr ? Json(nullptr) : Json(v->orders->index.base())},
        {"current_order", ReferenceOrderAccess::OrderFields(v->current_order)}, {"consist", ReferenceOrderAccess::Consist(*v)}});
    for (size_t slot = 0; slot < _order_backup_pool.first_unused; ++slot) {
        const OrderBackup *v = _order_backup_pool.data[slot];
        if (v != nullptr) backups.push_back(ReferenceOrderAccess::Backup(*v, slot));
    }
    return {{"lists", lists}, {"vehicles", vehicles}, {"backups", backups},
        {"list_pool", PoolState(_orderlist_pool)}, {"backup_pool", PoolState(_order_backup_pool)}};
}
inline Json AirportGeometry()
{
    const Json before = Snapshot();
    const auto random = _random, interactive = _interactive_random;
    const auto tick = TimerGameTick::counter;
    Json rows = Json::array();
    for (uint8_t type = 0; type < NEW_AIRPORT_OFFSET; ++type) {
        for (Direction direction : {DIR_N, DIR_E, DIR_S, DIR_W}) {
            Airport airport;
            airport.tile = TileXY(8, 8);
            airport.type = type;
            airport.rotation = direction;
            const AirportSpec *spec = airport.GetSpec();
            Json depots = Json::array();
            for (const auto &depot : spec->depots) depots.push_back({{"x", depot.ti.x}, {"y", depot.ti.y},
                {"hangar_num", depot.hangar_num}, {"tile", airport.GetRotatedTileFromOffset(depot.ti).base()}});
            rows.push_back({{"type", type}, {"rotation", direction}, {"base", airport.tile.base()},
                {"map_width", Map::SizeX()}, {"size_x", spec->size_x}, {"size_y", spec->size_y}, {"depots", depots}});
        }
    }
    Require(before == Snapshot(), "airport geometry changed order state");
    Require(tick == TimerGameTick::counter && random.state[0] == _random.state[0] && random.state[1] == _random.state[1] &&
        interactive.state[0] == _interactive_random.state[0] && interactive.state[1] == _interactive_random.state[1], "airport geometry changed clock/RNG");
    return {{"scope", "pure-original-offset-query-not-placed-airport"}, {"rows", rows}};
}
inline void Observe()
{
    const char *path = std::getenv("OTTD_ORDER_STATE_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    Write(path, {{"schema_version", 1}, {"role", Role()}, {"state", Snapshot()}, {"airport_geometry", AirportGeometry()}});
    afterload_ready = true;
}
}
#endif
