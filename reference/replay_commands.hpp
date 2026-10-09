// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_COMMANDS_HPP
#define OTTD_REFERENCE_REPLAY_COMMANDS_HPP
#include "../company_cmd.h"
#include "../landscape_cmd.h"
#include "../misc_cmd.h"
#include "../road_cmd.h"
#include "../terraform_cmd.h"
#include "../command_func.h"

namespace ReferenceReplay {
inline bool Post(const Json &command)
{
    const std::string kind = command.at("kind");
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
    Json result = {{"posted", false}, {"gate", nullptr}, {"test", nullptr}, {"exec", nullptr}, {"result", nullptr}};
    if (request.at("command").at("kind") == "terraform_land") result["returns"] = {{"test", nullptr}, {"exec", nullptr}, {"result", nullptr}};
    const CompanyID company(request.at("company").get<uint8_t>());
    AutoRestoreBackup current(_current_company, company);
    AutoRestoreBackup local(_local_company, company);
    AutoRestoreBackup shift(_shift_pressed, mode == "estimate");
    metadata = Json::object();
    receipt = &result;
    result["posted"] = Post(request.at("command"));
    receipt = nullptr;
    return result;
}
}
#endif
