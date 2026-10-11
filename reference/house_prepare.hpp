// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_HOUSE_PREPARE_HPP
#define OTTD_REFERENCE_HOUSE_PREPARE_HPP
#include "../town_cmd.h"
namespace ReferenceReplay {
inline bool HousePreparationEnabled()
{
    const char *enabled = std::getenv("OTTD_HOUSE_PREPARE");
    if (enabled == nullptr) return false;
    Require(std::string_view(enabled) == "1" && !_networking, "invalid original house preparation mode");
    return true;
}
inline bool HouseCommand(const Json &command)
{
    Require(HousePreparationEnabled(), "house commands require explicit original preparation mode");
    const std::string kind = command.at("kind");
    if (kind == "house_found_town") return Command<CMD_FOUND_TOWN>::Post(TileIndex(command.at("tile").get<uint32_t>()),
        static_cast<TownSize>(command.at("size").get<uint8_t>()), false, TL_ORIGINAL, false, uint32_t{0}, command.at("name").get<std::string>());
    if (kind == "house_growth_rate") return Command<CMD_TOWN_GROWTH_RATE>::Post(TownID(command.at("town").get<uint16_t>()), command.at("rate").get<uint16_t>());
    if (kind == "house_place") return Command<CMD_PLACE_HOUSE>::Post(TileIndex(command.at("tile").get<uint32_t>()),
        command.at("house").get<uint16_t>(), false, command.at("replace").get<bool>());
    if (kind == "house_setting") {
        const std::string name = command.at("name");
        Require(name == "economy.place_houses" || name == "economy.town_cargogen_mode" || name == "economy.town_cargo_scale" || name == "difficulty.competitors_interval", "house setting outside preparation allowlist");
        return Command<CMD_CHANGE_SETTING>::Post(name, command.at("value").get<int32_t>());
    }
    throw std::runtime_error("unknown original house preparation command");
}
}
#endif
