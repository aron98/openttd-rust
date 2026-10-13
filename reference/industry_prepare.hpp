// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_INDUSTRY_PREPARE_HPP
#define OTTD_REFERENCE_INDUSTRY_PREPARE_HPP
#include "../industry_cmd.h"
namespace ReferenceReplay {
inline bool IndustryCommand(const Json &command)
{
    const char *enabled = std::getenv("OTTD_INDUSTRY_PREPARE");
    Require(enabled != nullptr && std::string_view(enabled) == "1" && !_networking && _game_mode == GM_NORMAL,
        "industry preparation requires explicit offline normal mode");
    if (command.at("kind") == "industry_setting") {
        const std::string name = command.at("name");
        Require(name == "difficulty.industry_density" || name == "economy.industry_cargo_scale", "industry setting outside preparation allowlist");
        return Command<CMD_CHANGE_SETTING>::Post(name, command.at("value").get<int32_t>());
    }
    Require(command.at("kind") == "industry_build", "unknown industry preparation command");
    return Command<CMD_BUILD_INDUSTRY>::Post(TileIndex(command.at("tile").get<uint32_t>()),
        command.at("type").get<IndustryType>(), command.at("layout").get<uint32_t>(),
        command.at("fund").get<bool>(), command.at("seed").get<uint32_t>());
}
}
#endif
