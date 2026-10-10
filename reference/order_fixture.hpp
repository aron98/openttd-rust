#ifndef OTTD_REFERENCE_ORDER_FIXTURE_HPP
#define OTTD_REFERENCE_ORDER_FIXTURE_HPP
#include "reference_order_state.hpp"
#include "reference_replay.hpp"
#include "../command_func.h"
#include "../order_cmd.h"
#include "../order_func.h"
#include "../timetable_cmd.h"
#include "../vehicle_cmd.h"
#include "../landscape_cmd.h"
#include "../station_cmd.h"
#include "../road_cmd.h"
#include "../group_cmd.h"
#include "../core/backup_type.hpp"
#include "../company_base.h"
#include "saveload.h"

namespace ReferenceOrderFixture {
using ReferenceOrderState::Json;
using ReferenceOrderState::Require;
using ReferenceOrderState::Snapshot;
using ReferenceOrderState::Write;
inline Json plan, results;
inline size_t cursor = 0;
inline bool started = false, finished = false;
inline std::filesystem::path directory;
inline Vehicle *VehicleFor(const Json &a)
{
    Vehicle *v = Vehicle::GetIfValid(VehicleID(a.at("vehicle").get<uint32_t>()));
    Require(v != nullptr && v->type <= VEH_AIRCRAFT, "fixture vehicle is not an actual transport object");
    return v;
}
inline Order OrderFrom(const Json &a)
{
    Order order(a.at("type").get<uint8_t>(), a.at("flags").get<uint8_t>(), DestinationID(a.at("dest").get<uint16_t>()));
    order.SetRefit(a.at("refit_cargo").get<uint8_t>());
    order.SetWaitTime(a.at("wait_time").get<uint16_t>());
    order.SetTravelTime(a.at("travel_time").get<uint16_t>());
    order.SetMaxSpeed(a.at("max_speed").get<uint16_t>());
    return order;
}
inline Json Cost(const CommandCost &cost)
{
    return {{"success", cost.Succeeded()}, {"cost", static_cast<int64_t>(cost.GetCost())}, {"expenses", cost.GetExpensesType()}, {"error_id", cost.GetErrorMessage()}};
}
inline std::string Label(const Json &a)
{
    const std::string label = a.at("label").get<std::string>();
    Require(!label.empty() && label.find_first_not_of("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_") == std::string::npos, "invalid fixture label");
    return label;
}
inline Json Observation()
{
    if (std::getenv("OTTD_DEPOT_REMOVAL_OBSERVE") == nullptr) return Snapshot();
    Require(std::string(std::getenv("OTTD_DEPOT_REMOVAL_OBSERVE")) == "1", "invalid depot removal observer mode");
    Json tiles = Json::array();
    for (uint32_t index = 0; index < Map::Size(); ++index) tiles.push_back(ReferenceDepotRuntime::Parts(TileIndex(index)));
    return {{"orders", Snapshot()}, {"depot", ReferenceDepotRuntime::Snapshot()},
        {"vehicles", ReferenceRuntimeRoad::Live()}, {"tiles", std::move(tiles)}};
}
inline Json Save(const Json &a)
{
    const auto path = directory / (Label(a) + ".sav");
    Require(!std::filesystem::exists(path), "fixture save already exists");
    const Json before = Observation();
    Require(SaveOrLoad(path.string(), SLO_SAVE, DFT_GAME_FILE, NO_DIRECTORY, false) == SL_OK, "fixture native save failed");
    const Json after = Observation();
    Require(before == after, "saving changed initialized live order state");
    return {{"path", path.filename().string()}, {"before", before}, {"after", after}, {"role", ReferenceOrderState::Role()}};
}
inline Json Action(const Json &a)
{
    const std::string op = a.at("op").get<std::string>();
    if (op == "command") {
        Require(std::getenv("OTTD_DEPOT_REMOVAL_OBSERVE") != nullptr, "command fixture requires depot removal observer");
        Require(!_networking, "depot command fixture requires actual single player startup");
        const auto &request = a.at("request");
        const std::string kind = request.at("command").at("kind").get<std::string>();
        Require(kind == "landscape_clear" || kind == "build_road_depot", "unsupported depot lifecycle fixture command");
        Json receipt = ReferenceReplay::Execute(request);
        return {{"receipt", std::move(receipt)}, {"native_metadata", ReferenceReplay::metadata}};
    } else if (op == "backup") {
        OrderBackup::Backup(VehicleFor(a), a.at("user").get<uint32_t>());
    } else if (op == "backup_users") {
        const auto &users = a.at("users");
        Require(users.is_array() && users.size() <= 256, "invalid bounded backup users");
        Json states = Json::array();
        for (const auto &user : users) {
            OrderBackup::Backup(VehicleFor(a), user.get<uint32_t>());
            states.push_back({{"user", user}, {"pool", ReferenceOrderState::PoolState(_order_backup_pool)}});
        }
        return states;
    } else if (op == "reset_user") {
        OrderBackup::ResetOfUser(a.contains("tile") ? TileIndex(a.at("tile").get<uint32_t>()) : INVALID_TILE, a.at("user").get<uint32_t>());
    } else if (op == "reset_tile") {
        OrderBackup::Reset(TileIndex(a.at("tile").get<uint32_t>()), false);
    } else if (op == "clear_group") {
        OrderBackup::ClearGroup(GroupID(a.at("group").get<uint16_t>()));
    } else if (op == "clear_vehicle") {
        OrderBackup::ClearVehicle(VehicleFor(a));
    } else if (op == "invalidate_depot") {
        OrderBackup::Reset(TileIndex(a.at("tile").get<uint32_t>()), false);
        RemoveOrderFromAllVehicles(OT_GOTO_DEPOT, DestinationID(a.at("depot_id").get<uint16_t>()), false);
    } else if (op == "insert_order") {
        Vehicle *v = VehicleFor(a);
        Require(v->orders != nullptr || OrderList::CanAllocateItem(), "order fixture list pool full");
        InsertOrder(v, OrderFrom(a.at("order")), a.at("index").get<uint8_t>());
    } else if (op == "orphan_order") {
        Require(OrderList::CanAllocateItem(), "orphan fixture list pool full");
        OrderList *list = new OrderList();
        list->InsertOrderAt(OrderFrom(a.at("order")), 0);
        return {{"list", list->index.base()}, {"scope", "unreferenced-saved-record-admission-probe"}};
    } else if (op == "current_order") {
        VehicleFor(a)->current_order = OrderFrom(a.at("order"));
    } else if (op == "delete_orders") {
        DeleteVehicleOrders(VehicleFor(a));
    } else if (op == "share") {
        Vehicle *v = VehicleFor(a);
        AutoRestoreBackup company(_current_company, v->owner);
        const uint8_t mode = a.value("mode", uint8_t(CO_SHARE));
        Require(mode <= CO_UNSHARE, "invalid order clone fixture mode");
        const auto cost = Command<CMD_CLONE_ORDER>::Do(DoCommandFlag::Execute, static_cast<CloneOptions>(mode), v->index, VehicleID(a.at("source").get<uint32_t>()));
        Require(cost.Succeeded(), "original share command failed");
        return Cost(cost);
    } else if (op == "consist_fields") {
        Vehicle *v = VehicleFor(a);
        const auto &fields = a.at("fields");
        v->name = fields.at("name").get<std::string>();
        v->current_order_time = fields.at("current_order_time").get<int32_t>();
        v->lateness_counter = fields.at("lateness_counter").get<int32_t>();
        v->timetable_start = fields.at("timetable_start").get<uint64_t>();
        v->service_interval = fields.at("service_interval").get<uint16_t>();
        v->cur_real_order_index = fields.at("cur_real_order_index").get<uint8_t>();
        v->cur_implicit_order_index = fields.at("cur_implicit_order_index").get<uint8_t>();
        v->vehicle_flags = VehicleFlags(fields.at("vehicle_flags").get<uint16_t>());
    } else if (op == "build_vehicle") {
        AutoRestoreBackup company(_current_company, CompanyID(a.at("company").get<uint8_t>()));
        const auto [cost, vehicle, capacity, mail, capacities] = Command<CMD_BUILD_VEHICLE>::Do(DoCommandFlag::Execute,
            TileIndex(a.at("tile").get<uint32_t>()), EngineID(a.at("engine").get<uint16_t>()), true, INVALID_CARGO, CLIENT_ID_SERVER);
        Require(cost.Succeeded(), "original vehicle construction failed");
        return {{"cost", Cost(cost)}, {"vehicle", vehicle.base()}, {"capacity", capacity}, {"mail", mail}, {"capacities", capacities}};
    } else if (op == "build_depot") {
        AutoRestoreBackup company(_current_company, CompanyID(a.at("company").get<uint8_t>()));
        const auto cost = Command<CMD_BUILD_ROAD_DEPOT>::Do(DoCommandFlag::Execute,
            TileIndex(a.at("tile").get<uint32_t>()), ROADTYPE_ROAD, DIAGDIR_NE);
        Require(cost.Succeeded(), "original depot construction failed");
        return Cost(cost);
    } else if (op == "create_group") {
        AutoRestoreBackup company(_current_company, CompanyID(a.at("company").get<uint8_t>()));
        const auto [cost, id] = Command<CMD_CREATE_GROUP>::Do(DoCommandFlag::Execute, VEH_ROAD, GroupID::Invalid());
        Require(cost.Succeeded(), "original group construction failed");
        return {{"cost", Cost(cost)}, {"group", id.base()}};
    } else if (op == "add_group") {
        Vehicle *v = VehicleFor(a);
        AutoRestoreBackup company(_current_company, v->owner);
        const auto [cost, id] = Command<CMD_ADD_VEHICLE_GROUP>::Do(DoCommandFlag::Execute,
            GroupID(a.at("group").get<uint16_t>()), v->index, false, VehicleListIdentifier{});
        Require(cost.Succeeded(), "original group assignment failed");
        return {{"cost", Cost(cost)}, {"group", id.base()}};
    } else if (op == "timetable") {
        Vehicle *v = VehicleFor(a);
        AutoRestoreBackup company(_current_company, v->owner);
        const uint8_t field = a.at("field").get<uint8_t>();
        Require(field < MTF_END, "invalid timetable field");
        const auto cost = Command<CMD_CHANGE_TIMETABLE>::Do(DoCommandFlag::Execute, v->index,
            a.at("index").get<uint8_t>(), static_cast<ModifyTimetableFlags>(field), a.at("value").get<uint16_t>());
        Require(cost.Succeeded(), "original timetable assignment failed");
        return Cost(cost);
    } else if (op == "build_airport") {
        AutoRestoreBackup company(_current_company, CompanyID(a.at("company").get<uint8_t>()));
        const auto cost = Command<CMD_BUILD_AIRPORT>::Do(DoCommandFlag::Execute,
            TileIndex(a.at("tile").get<uint32_t>()), a.at("airport_type").get<uint8_t>(), a.at("layout").get<uint8_t>(), StationID::Invalid(), true);
        Require(cost.Succeeded(), "original airport construction failed");
        return Cost(cost);
    } else if (op == "clear_depot") {
        AutoRestoreBackup company(_current_company, CompanyID(a.at("company").get<uint8_t>()));
        DoCommandFlags flags;
        if (a.value("execute", false)) flags.Set(DoCommandFlag::Execute);
        if (a.value("automatic", false)) flags.Set(DoCommandFlag::Auto);
        return Cost(Command<CMD_LANDSCAPE_CLEAR>::Do(flags, TileIndex(a.at("tile").get<uint32_t>())));
    } else if (op == "save") {
        return Save(a);
    } else if (op != "snapshot" && op != "await_file") {
        ReferenceOrderState::HostError("unknown fixture operation");
    }
    return nullptr;
}
inline void Poll()
{
    const char *path = std::getenv("OTTD_ORDER_FIXTURE_PATH");
    if (path == nullptr || finished || !ReferenceOrderState::afterload_ready || _game_mode != GM_NORMAL) return;
    if (!started) {
        Require(std::filesystem::file_size(path) <= 262144, "fixture input exceeds bound");
        std::ifstream input(path);
        plan = Json::parse(input);
        Require(plan.at("schema_version") == 1 && plan.at("actions").is_array() && plan.at("actions").size() <= 4096, "invalid fixture protocol");
        const std::string role = plan.at("role").get<std::string>();
        Require((role == "sp" && !_networking) || (role == "server" && _networking && _network_server && _network_dedicated) ||
            (role == "client" && _networking && !_network_server), "actual native role differs from requested fixture");
        const char *destination = std::getenv("OTTD_ORDER_FIXTURE_DIR");
        Require(destination != nullptr, "missing fixture output directory");
        directory = destination;
        Require(!std::filesystem::exists(directory), "fixture output already exists");
        std::filesystem::create_directories(directory);
        results = {{"schema_version", 1}, {"case", plan.at("case")}, {"role", ReferenceOrderState::Role()}, {"initial", Observation()}, {"actions", Json::array()}};
        started = true;
    }
    Require(_pause_mode.Test(PauseMode::Normal), "order fixture requires actual paused source");
    if (cursor < plan.at("actions").size()) {
        const auto &action = plan.at("actions").at(cursor);
        if (action.at("op") == "await_file" && !std::filesystem::exists(directory / (Label(action) + ".ready"))) return;
        const Json before = Observation();
        const Json result = Action(action);
        results["actions"].push_back({{"index", cursor}, {"input", action}, {"before", before}, {"result", result}, {"after", Observation()}, {"role", ReferenceOrderState::Role()}});
        ++cursor;
        return;
    }
    results["final"] = Observation();
    results["final_role"] = ReferenceOrderState::Role();
    Write(directory / "results.json", results);
    finished = true;
    if (plan.value("exit", true)) _exit_game = true;
}
}
#endif
