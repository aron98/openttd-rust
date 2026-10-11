// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ROAD_STATION_PREPARE_HPP
#define OTTD_REFERENCE_ROAD_STATION_PREPARE_HPP
#include "../tree_cmd.h"

namespace ReferenceReplay {
inline bool PrepareRoadStation(const Json &command)
{
    Require(std::getenv("OTTD_ROAD_STATION_PREPARE") != nullptr && !_networking, "station preparation requires explicit offline mode");
    const std::string kind = command.at("kind");
    if (kind == "road_station_prepare_company") {
        Require(!Company::IsValidID(CompanyID(1)), "station company already exists");
        return Command<CMD_COMPANY_CTRL>::Post(CCA_NEW_AI, CompanyID(1), CRR_NONE, INVALID_CLIENT_ID);
    }
    if (kind == "road_station_prepare_tree") {
        const TileIndex tile(command.at("tile").get<uint32_t>());
        return Command<CMD_PLANT_TREE>::Post(tile, tile, uint8_t{0}, false);
    }
    if (kind == "road_station_prepare_money") {
        const int64_t amount = command.at("amount").get<int64_t>();
        Require(amount < 0, "station cash witness must remove money");
        return Command<CMD_MONEY_CHEAT>::Post(Money(amount));
    }
    if (kind == "road_station_prepare_setting") {
        const std::string name = command.at("name");
        const int32_t value = command.at("value");
        Require((name == "station.modified_catchment" || name == "station.distant_join_stations") && (value == 0 || value == 1), "station setting outside witness domain");
        return Command<CMD_CHANGE_SETTING>::Post(name, value);
    }
    throw std::runtime_error("unknown station preparation command");
}
}
#endif
