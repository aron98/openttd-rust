// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_COMMANDS_HPP
#define OTTD_REFERENCE_REPLAY_COMMANDS_HPP
#include "../company_cmd.h"
#include "../landscape_cmd.h"
#include "../misc_cmd.h"
#include "../road_cmd.h"
#include "../terraform_cmd.h"
#include "../vehicle_cmd.h"
#include "../command_func.h"

namespace ReferenceReplay {
inline bool Post(const Json &command)
{
    const std::string kind = command.at("kind");
    if (kind == "build_road_depot") return Command<CMD_BUILD_ROAD_DEPOT>::Post(TileIndex(command.at("tile").get<uint32_t>()), static_cast<RoadType>(command.at("road_type").get<uint8_t>()), static_cast<DiagDirection>(command.at("direction").get<uint8_t>()));
    if (kind == "sell_vehicle") return Command<CMD_SELL_VEHICLE>::Post(TileIndex(command.at("location").get<uint32_t>()), VehicleID(command.at("vehicle").get<uint32_t>()), command.at("sell_chain").get<bool>(), command.at("backup_order").get<bool>(), ClientID(command.at("client_id").get<uint32_t>()));
    if (kind == "build_vehicle") return Command<CMD_BUILD_VEHICLE>::Post(TileIndex(command.at("tile").get<uint32_t>()), EngineID(command.at("engine").get<uint16_t>()), command.at("use_free_vehicles").get<bool>(), command.at("cargo").get<CargoType>(), ClientID(command.at("client_id").get<uint32_t>()));
    if (kind == "change_service_interval") return Command<CMD_CHANGE_SERVICE_INT>::Post(VehicleID(command.at("vehicle").get<uint32_t>()), command.at("interval").get<uint16_t>(), command.at("custom").get<bool>(), command.at("percent").get<bool>());
    if (kind == "level_land") return Command<CMD_LEVEL_LAND>::Post(TileIndex(command.at("tile").get<uint32_t>()), TileIndex(command.at("start_tile").get<uint32_t>()), command.at("diagonal").get<bool>(), static_cast<LevelMode>(command.at("level_mode").get<uint8_t>()));
    if (kind == "terraform_land") return Command<CMD_TERRAFORM_LAND>::Post(TileIndex(command.at("tile").get<uint32_t>()), static_cast<Slope>(command.at("slope").get<uint8_t>()), command.at("dir_up").get<bool>());
    if (kind == "build_road") return Command<CMD_BUILD_ROAD>::Post(TileIndex(command.at("tile").get<uint32_t>()),
        static_cast<RoadBits>(command.at("pieces").get<uint8_t>()), static_cast<RoadType>(command.at("road_type").get<uint8_t>()),
        static_cast<DisallowedRoadDirections>(command.at("toggle_disallowed").get<uint8_t>()), TownID(command.at("town_id").get<uint16_t>()));
    if (kind == "landscape_clear") return Command<CMD_LANDSCAPE_CLEAR>::Post(TileIndex(command.at("tile").get<uint32_t>()));
    if (kind == "increase_loan") return Command<CMD_INCREASE_LOAN>::Post(static_cast<LoanCommand>(command.at("method").get<uint8_t>()), Money(command.at("amount").get<int64_t>()));
    if (kind == "decrease_loan") return Command<CMD_DECREASE_LOAN>::Post(static_cast<LoanCommand>(command.at("method").get<uint8_t>()), Money(command.at("amount").get<int64_t>()));
    if (kind == "rename_company") return Command<CMD_RENAME_COMPANY>::Post(command.at("text").get<std::string>());
    if (kind == "rename_president") return Command<CMD_RENAME_PRESIDENT>::Post(command.at("text").get<std::string>());
    if (kind == "pause") return Command<CMD_PAUSE>::Post(static_cast<PauseMode>(command.at("mode").get<uint8_t>()), command.at("paused").get<bool>());
    throw std::runtime_error("unsupported native replay command: " + kind);
}
inline Json Execute(const Json &request)
{
    const std::string mode = request.at("mode");
    Require(mode == "post" || mode == "estimate", "invalid command request mode");
    const bool observe_trees = TreeRatingEnabled();
    if (observe_trees) {
        const std::string kind = request.at("command").at("kind");
        Require(kind == "landscape_clear" || kind == "terraform_land" || kind == "level_land", "unsupported tree rating observer command");
        Require(!_networking, "tree rating observer requires offline replay");
    }
    Json result = {{"posted", false}, {"gate", nullptr}, {"test", nullptr}, {"exec", nullptr}, {"result", nullptr}};
    if (request.at("command").at("kind") == "terraform_land" || request.at("command").at("kind") == "level_land" || request.at("command").at("kind") == "build_vehicle") result["returns"] = {{"test", nullptr}, {"exec", nullptr}, {"result", nullptr}};
    const CompanyID company(request.at("company").get<uint8_t>());
    AutoRestoreBackup current(_current_company, company);
    AutoRestoreBackup local(_local_company, company);
    AutoRestoreBackup shift(_shift_pressed, mode == "estimate");
    metadata = Json::object();
    if (std::getenv("OTTD_ROAD_SALE_OBSERVE") != nullptr && (request.at("command").at("kind") == "build_vehicle" || request.at("command").at("kind") == "sell_vehicle")) metadata["sale_before"] = ReferenceRuntimeRoad::SaleSnapshot();
    if (request.at("command").at("kind") == "build_vehicle") metadata["purchase_before"] = ReferenceRuntimeRoad::Live();
    if (std::getenv("OTTD_DEPOT_LIVE") != nullptr) metadata["depot_before"] = {{"depot", ReferenceDepotRuntime::Snapshot()}, {"vehicles", ReferenceRuntimeRoad::Live()}};
    {
        AutoRestoreBackup restore_receipt(receipt, &result);
        TreeRatingCapture tree_capture(observe_trees);
        result["posted"] = Post(request.at("command"));
        if (observe_trees) metadata["tree_rating"] = tree_capture.Finish();
    }
    return result;
}
}
#endif
