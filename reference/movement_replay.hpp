// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_REPLAY_HPP
#define OTTD_REFERENCE_MOVEMENT_REPLAY_HPP
#include "reference_movement_sink.hpp"
#include "reference_movement_admission.hpp"
#include "reference_movement_protocol.hpp"
#include "reference_movement_prepare.hpp"
#include "../reference_movement_baseline.hpp"
namespace ReferenceMovement {
inline uint64_t DirectoryBytes(const std::filesystem::path &directory)
{
    uint64_t size = 0;
    for (const auto &entry : std::filesystem::recursive_directory_iterator(directory)) {
        ReferenceWorld::Require(!entry.is_symlink(), "movement output symlink forbidden");
        if (entry.is_regular_file()) size += entry.file_size();
    }
    return size;
}
inline void Checkpoint(const std::filesystem::path &directory, const std::string &label, Json &result)
{
    Emit("checkpoint", "enter");
    const Json before = {{"live", Live()}, {"spatial", Spatial()}};
    AutoRestoreBackup checkpoint_context(internal_checkpoint, true);
    const std::string content = (directory / (label + ".content.json")).string();
    setenv("OTTD_CONTENT_PATH", content.c_str(), 1);
    result["checkpoints"].push_back(ReferenceReplay::Checkpoint(directory, label));
    unsetenv("OTTD_CONTENT_PATH");
    const Json after = {{"live", Live()}, {"spatial", Spatial()}};
    ReferenceReplay::Write(directory / (label + ".movement.json"), {{"before_save", before}, {"after_save", after}});
    ReferenceWorld::Require(before == after, "native save changed observed movement state");
    Emit("checkpoint", "leave");
}
inline bool MovingWitness(const RoadVehicle *vehicle, const Json &tiles)
{
    if (vehicle->vehstatus.Any({VehState::Stopped, VehState::Hidden, VehState::Crashed}) || vehicle->cur_speed == 0) return false;
    if (!IsTileType(vehicle->tile, MP_ROAD) || !IsNormalRoad(vehicle->tile) || vehicle->dest_tile != TileIndex{}) return false;
    for (const Json &tile : tiles) if (Bounded(tile, Map::Size() - 1) == vehicle->tile.base()) return true;
    return false;
}
inline Json Command(const Json &request)
{
    ReferenceWorld::Require(request.at("mode") == "post", "movement preparer requires actual command post");
    const Json before = PreparationState();
    Json value = ReferenceReplay::Execute(request);
    return {{"action", action}, {"request", request}, {"receipt", value}, {"metadata", ReferenceReplay::metadata},
        {"before", before}, {"after", PreparationState()}};
}
inline bool CommandSucceeded(const Json &value)
{
    const Json &receipt = value.at("receipt");
    return receipt.at("posted") == true && receipt.at("gate").is_null()
        && !receipt.at("test").is_null() && receipt.at("test").at("success") == true
        && !receipt.at("exec").is_null() && receipt.at("exec").at("success") == true
        && !receipt.at("result").is_null() && receipt.at("result").at("success") == true;
}
inline bool Run()
{
    static bool active = false;
    static bool done = false;
    if (active || done || _game_mode != GM_NORMAL) return false;
    using ReferenceWorld::Require;
    ModeGuard();
    const std::filesystem::path directory(std::getenv("OTTD_REPLAY_OUTPUT"));
    Require(std::filesystem::is_directory(directory), "movement output directory must be created by driver");
    for (const char *name : {"results.json", "events.jsonl", "initial.sav"}) Require(!std::filesystem::exists(directory / name), "stale movement output");
    std::ifstream input(std::getenv("OTTD_REPLAY_PATH"));
    Require(input.good(), "movement descriptor unavailable");
    const Json protocol = Json::parse(input);
    ProtocolGuard(protocol);
    ReferenceMovementBaseline::RequireComplete();
    const std::string mode = protocol.at("mode");
    Require(mode == "prepare" || mode == "replay" || mode == "discover", "movement protocol mode");
    const bool prepare = mode == "prepare";
    const bool discover = mode == "discover";
    Require(prepare == (std::getenv("OTTD_MOVEMENT_PREPARE") != nullptr), "movement preparation mode mismatch");
    const uint64_t max_calls = Bounded(protocol.at("max_calls"), 10000);
    const uint64_t max_bytes = Bounded(protocol.at("max_bytes"), uint64_t{8} << 30);
    const uint64_t max_seconds = Bounded(protocol.at("max_seconds"), 3600);
    const uint64_t event_limit = Bounded(protocol.at("max_events"), 10000000);
    Require(max_bytes > 0 && max_seconds > 0 && event_limit > 0, "movement zero host budget");
    const uint64_t requested_calls = prepare || discover ? max_calls : Bounded(protocol.at("calls"), max_calls);
    Require(!protocol.contains("fixture") && !protocol.contains("actions"), "movement cannot use synthetic replay fixture");
    Require(prepare || !protocol.contains("commands"), "loaded movement replay forbids commands");
    Require(prepare || !protocol.contains("prepare_settings"), "loaded movement replay forbids prepare_settings");
    if (prepare) {
        const Json &settings = protocol.at("prepare_settings");
        Require(settings.is_array() && settings.size() == 1, "movement preparation requires one setting command");
        const Json &request = settings.at(0);
        const auto company = static_cast<uint8_t>(Bounded(request.at("company"), 254));
        Require(Company::IsValidID(CompanyID(company)), "movement setting company invalid");
        Require(request.at("command").at("value").is_number_integer(), "movement setting integer value required");
        Require(request == Json{{"company", company}, {"mode", "post"}, {"command", {
            {"kind", "movement_prepare_original_acceleration"}, {"name", "vehicle.roadveh_acceleration_model"}, {"value", 0}}}}, "movement prepare_settings request not whitelisted");
    }
    if (prepare) Require(protocol.at("witness_tiles").is_array() && !protocol.at("witness_tiles").empty(), "movement witness tile list required");
    Json result = {{"schema_version", 2}, {"kind", "road_movement"}, {"mode", mode},
        {"outcome", "incomplete"}, {"commands", Json::array()}, {"checkpoints", Json::array()},
        {"crossings", Json::array()}, {"subject", nullptr}, {"calls", 0}, {"trace", protocol.at("trace")}};
    active = true;
    const auto started = std::chrono::steady_clock::now();
    uint64_t command_count = 0;
    const auto check_budget = [&]() {
        if (failure != Failure::None) result["outcome"] = FailureName();
        else if (DirectoryBytes(directory) >= max_bytes) result["outcome"] = "host_storage_budget";
        else if (std::chrono::steady_clock::now() - started >= std::chrono::seconds(max_seconds)) result["outcome"] = "host_time_budget";
        return result.at("outcome") == "incomplete";
    };
    const auto execute = [&](const Json &request) -> Json {
        Require(check_budget(), "host budget forbids command");
        Require(command_count < 179, "movement command host bound");
        ++command_count;
        ++action;
        Json receipt = Command(request);
        result["command_count"] = command_count;
        if (!CommandSucceeded(receipt)) result["outcome"] = "native_command_rejected";
        check_budget();
        return receipt;
    };
    const char *stage = "admission";
    try {
        SinkLifetime lifetime(directory, event_limit, max_bytes, protocol.at("trace").get<bool>());
        if (prepare) PrepareSeedGuard();
        else WorldGuard();
        Require(prepare ? Vehicle::GetNumItems() == 0 : Vehicle::GetNumItems() == 1, "movement initial vehicle count");
        uint32_t subject = prepare ? UINT32_MAX : static_cast<uint32_t>(Bounded(protocol.at("subject"), UINT32_MAX - 1));
        if (!prepare) {
            const RoadVehicle *vehicle = Subject(subject);
            Require(vehicle->dest_tile == TileIndex{} && IsTileType(vehicle->tile, MP_ROAD) && IsNormalRoad(vehicle->tile), "loaded movement orderless road required");
            if (discover) Require(vehicle->cur_speed > 0 && !vehicle->vehstatus.Any({VehState::Stopped, VehState::Hidden}), "crossing discovery requires moving subject");
        }
        stage = "checkpoint";
        Checkpoint(directory, "initial", result);
        if (prepare) {
            check_budget();
            if (result.at("outcome") == "incomplete") {
                stage = "prepare_settings";
                const auto before = _settings_game.vehicle.roadveh_acceleration_model;
                Json receipt = execute(protocol.at("prepare_settings").at(0));
                receipt["value_before"] = before;
                receipt["value_after"] = _settings_game.vehicle.roadveh_acceleration_model;
                result["prepare_settings"] = Json::array({receipt});
                stage = "checkpoint";
                if (DirectoryBytes(directory) < max_bytes) Checkpoint(directory, "settings_after", result);
                check_budget();
                if (result.at("outcome") == "incomplete") {
                    stage = "prepare_settings";
                    Require(Vehicle::GetNumItems() == 0, "setting command populated vehicle pool");
                    WorldGuard();
                }
            }
        }
        if (prepare && result.at("outcome") == "incomplete") {
            stage = "commands";
            const Json &commands = protocol.at("commands");
            Require(commands.is_array() && commands.size() > 0 && commands.size() <= 256, "movement preparer command roster");
            const uint64_t reservations = Bounded(protocol.at("reservation_count"), 75);
            std::vector<uint32_t> reserved;
            for (const Json &request : commands) {
                if (!check_budget()) break;
                const std::string kind = request.at("command").at("kind");
                if (kind == "build_vehicle") {
                    result["purchase_feasibility"] = PurchaseFeasibility(request, reservations);
                    const Json &feasible = result.at("purchase_feasibility");
                    if (feasible.at("buildable") != true || feasible.at("affordable_before_sales") != true) {
                        result["outcome"] = "native_purchase_unavailable_or_unaffordable";
                        break;
                    }
                    for (uint64_t i = 0; i < reservations && check_budget(); ++i) {
                        Json receipt = execute(request);
                        receipt["role"] = "reservation";
                        result["commands"].push_back(receipt);
                        if (!CommandSucceeded(receipt)) break;
                        const uint32_t id = PurchasedID(receipt);
                        reserved.push_back(id);
                        ReservedVehicle(id);
                        Require(id == i, "original reservation allocation differs from portfolio");
                    }
                    if (!check_budget()) break;
                }
                Json receipt = execute(request);
                receipt["role"] = kind == "build_vehicle" ? "target" : "construction";
                result["commands"].push_back(receipt);
                if (!CommandSucceeded(receipt)) break;
                if (kind == "build_vehicle") {
                    subject = PurchasedID(receipt);
                    result["subject"] = subject;
                    ReservedVehicle(subject);
                    Require(subject == reservations, "original target allocation differs from portfolio");
                    for (uint32_t id : reserved) {
                        if (!check_budget()) break;
                        ReservedVehicle(id);
                        Json sale = execute(PostRequest({{"kind", "sell_vehicle"}, {"location", 673},
                            {"vehicle", id}, {"sell_chain", false}, {"backup_order", false}, {"client_id", 0}}));
                        sale["role"] = "reservation_release";
                        result["commands"].push_back(sale);
                        if (!CommandSucceeded(sale)) break;
                        Require(!Vehicle::IsValidID(VehicleID(id)), "successful reservation sale retained vehicle");
                    }
                }
            }
            if (result.at("outcome") == "incomplete") {
                Require(subject != UINT32_MAX, "movement preparer missing successful build vehicle");
                const RoadVehicle *vehicle = Subject(subject);
                Require(vehicle->IsStoppedInDepot(), "movement freshly built vehicle not stopped in depot");
                result["depot"] = vehicle->tile.base();
                result["subject"] = subject;
                stage = "checkpoint";
                Checkpoint(directory, "built", result);
                if (!check_budget()) throw std::runtime_error("host budget after built checkpoint");
                stage = "commands";
                const Json receipt = execute({{"company", vehicle->owner.base()}, {"mode", "post"},
                    {"command", {{"kind", "movement_prepare_start_stop"}, {"vehicle", subject}}}});
                result["commands"].push_back(receipt);
                if (CommandSucceeded(receipt)) Require(!Subject(subject)->vehstatus.Test(VehState::Stopped), "native StartStop did not start subject");
            }
        } else if (!prepare) {
            result["subject"] = subject;
        }
        if (result.at("outcome") == "incomplete") {
            uint32_t previous_tile = Subject(subject)->tile.base();
            int32_t previous_x = Subject(subject)->x_pos;
            int32_t previous_y = Subject(subject)->y_pos;
            for (uint64_t step = 0; step < requested_calls; ++step) {
                if (std::chrono::steady_clock::now() - started >= std::chrono::seconds(max_seconds)) { result["outcome"] = "host_time_budget"; break; }
                if (DirectoryBytes(directory) >= max_bytes) { result["outcome"] = "host_storage_budget"; break; }
                if (failure != Failure::None) { result["outcome"] = FailureName(); break; }
                ++action;
                ++call;
                stage = "native_tick";
                StateGameLoop();
                result["calls"] = call;
                stage = "checkpoint";
                Checkpoint(directory, "tick_" + std::to_string(call), result);
                if (failure != Failure::None) { result["outcome"] = FailureName(); break; }
                if (DirectoryBytes(directory) >= max_bytes) { result["outcome"] = "host_storage_budget"; break; }
                stage = "reached_domain";
                WorldGuard();
                const RoadVehicle *vehicle = Subject(subject);
                Require(vehicle->dest_tile == TileIndex{}, "movement reached service/destination branch");
                if (vehicle->tile.base() != previous_tile) {
                    Require(vehicle->x_pos != previous_x || vehicle->y_pos != previous_y, "tile changed without displacement");
                    result["crossings"].push_back({{"call", call}, {"from", previous_tile}, {"to", vehicle->tile.base()}});
                    if (discover) {
                        Require(IsTileType(vehicle->tile, MP_ROAD) && IsNormalRoad(vehicle->tile), "crossing discovery reached depot/stop");
                        result["outcome"] = "native_crossing_witness";
                        result["witness_label"] = "tick_" + std::to_string(call);
                        break;
                    }
                }
                previous_tile = vehicle->tile.base();
                previous_x = vehicle->x_pos;
                previous_y = vehicle->y_pos;
                if (prepare && MovingWitness(vehicle, protocol.at("witness_tiles"))) {
                    result["outcome"] = "native_moving_witness";
                    result["witness_label"] = "tick_" + std::to_string(call);
                    break;
                }
            }
            if (result.at("outcome") == "incomplete") result["outcome"] = prepare || discover ? "host_tick_budget" : "native_completed";
        }
        stage = "checkpoint";
        if (DirectoryBytes(directory) < max_bytes) Checkpoint(directory, "final", result);
        if (failure != Failure::None) result["outcome"] = FailureName();
        if (DirectoryBytes(directory) >= max_bytes) result["outcome"] = "host_storage_budget";
        check_budget();
        result["events"] = sequence;
        result["event_bytes"] = event_bytes;
    } catch (const std::exception &error) {
        if (result.at("outcome") == "incomplete") result["outcome"] = std::string_view(stage) == "native_tick" ? "native_exception" :
            std::string_view(stage) == "reached_domain" ? "pilot_domain_reached" : "observer_or_admission_failure";
        result["stage"] = stage;
        result["error"] = error.what();
        result["events"] = sequence;
        result["event_bytes"] = event_bytes;
    }
    sink = nullptr;
    for (const char *name : {"OTTD_CONTENT_PATH", "OTTD_WORLD_PATH", "OTTD_WORLD_SCHEMA_PATH", "OTTD_WORLD_DERIVED_PATH"}) unsetenv(name);
    result["observer_failure"] = FailureName();
    result["directory_bytes_before_results"] = DirectoryBytes(directory);
    ReferenceReplay::Write(directory / "results.json", result);
    _settings_client.gui.autosave_on_exit = false;
    done = true;
    active = false;
    _exit_game = true;
    return true;
}
}
#endif
