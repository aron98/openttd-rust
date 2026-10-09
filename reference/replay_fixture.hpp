// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_FIXTURE_HPP
#define OTTD_REFERENCE_REPLAY_FIXTURE_HPP
#include "../ai/ai.hpp"
#include "../game/game.hpp"
#include "../clear_map.h"
#include "../animated_tile_func.h"
#include "../void_map.h"
#include "../object_base.h"
#include "../economy_func.h"
#include "../company_base.h"
#include "../timer/timer.h"
#include "../linkgraph/linkgraphschedule.h"
#include "../linkgraph/linkgraphjob.h"
#include "../tunnelbridge_cmd.h"
#include "../bridge_map.h"

extern TimeoutTimer<TimerGameTick> _new_competitor_timeout;
extern uint16_t _disaster_delay;
extern TileIndex _cur_tileloop_tile;
Company *DoStartupNewCompany(bool is_ai, CompanyID company);

namespace ReferenceReplay {
inline void PrepareTerraform(const Json &setup)
{
    Require(Map::SizeX() == 256 && Map::SizeY() == 256, "terraform fixture requires populated 256-square world");
    for (const Vehicle *vehicle : Vehicle::Iterate()) {
        Require(vehicle->x_pos < 0 || vehicle->y_pos < 0 || vehicle->x_pos >= 17 * TILE_SIZE || vehicle->y_pos >= 17 * TILE_SIZE, "terraform pad overlaps vehicle position");
    }
    for (uint y = 0; y <= 16; ++y) for (uint x = 0; x <= 16; ++x) {
        const TileIndex tile = TileXY(x, y);
        Require(IsTileType(tile, MP_CLEAR) || IsTileType(tile, MP_TREES) || ((IsWaterTile(tile) || IsCoastTile(tile)) && GetWaterClass(tile) == WaterClass::Sea), "terraform pad overlaps infrastructure");
        MakeClear(tile, setup.value("bare", false) ? CLEAR_GRASS : static_cast<ClearGround>((x + y) % 3), setup.value("bare", false) ? 0 : (x + 2 * y) % 4);
        SetTileHeight(tile, std::min({x, y, 16 - x, 16 - y, 4U}));
    }
    const bool freeform = setup.value("freeform", false);
    if (freeform) {
        for (const Vehicle *vehicle : Vehicle::Iterate()) Require(vehicle->x_pos < 0 || vehicle->y_pos < 0 || (vehicle->x_pos >= TILE_SIZE && vehicle->y_pos >= TILE_SIZE), "terraform freeform border overlaps vehicle position");
        for (uint i = 0; i < Map::SizeX(); ++i) {
            Require(IsTileType(TileXY(i, Map::MaxY()), MP_VOID) && IsTileType(TileXY(Map::MaxX(), i), MP_VOID), "terraform lower border is not void");
            for (TileIndex tile : {TileXY(i, 0), TileXY(0, i)}) {
                Require(!IsBridgeAbove(tile) && (IsTileType(tile, MP_VOID) || IsTileType(tile, MP_CLEAR) || ((IsWaterTile(tile) || IsCoastTile(tile)) && GetWaterClass(tile) == WaterClass::Sea)), "terraform freeform border overlaps infrastructure");
            }
        }
    }
    _settings_game.construction.freeform_edges = freeform;
    _settings_game.difficulty.construction_cost = 1;
    _economy.inflation_prices = 1 << 16;
    RecomputePrices();
    if (freeform) {
        for (uint i = 0; i < Map::SizeX(); ++i) for (TileIndex tile : {TileXY(i, 0), TileXY(0, i)}) {
            const uint height = TileHeight(tile);
            MakeVoid(tile);
            SetTileHeight(tile, height);
        }
    }
    if (setup.contains("height_limit")) _settings_game.construction.map_height_limit = setup.at("height_limit").get<uint8_t>();
    if (setup.value("high_hill", false)) {
        for (const Vehicle *vehicle : Vehicle::Iterate()) Require(vehicle->x_pos < 56 * TILE_SIZE || vehicle->x_pos >= 89 * TILE_SIZE || vehicle->y_pos < 0 || vehicle->y_pos >= 33 * TILE_SIZE, "terraform hill overlaps vehicle position");
        for (int y = 0; y <= 32; ++y) for (int x = 56; x <= 88; ++x) {
            const TileIndex tile = TileXY(x, y);
            const int height = std::max(static_cast<int>(TileHeight(tile)), 15 - std::abs(x - 72) - std::abs(y - 16));
            if (height == TileHeight(tile)) continue;
            Require(IsTileType(tile, MP_CLEAR) || IsTileType(tile, MP_TREES) || ((IsWaterTile(tile) || IsCoastTile(tile)) && GetWaterClass(tile) == WaterClass::Sea), "terraform hill overlaps infrastructure");
            MakeClear(tile, CLEAR_GRASS, 3);
            SetTileHeight(tile, height);
        }
        for (uint y = 0; y <= 32; ++y) for (uint x = 56; x <= 88; ++x) {
            const int height = TileHeight(TileXY(x, y));
            Require(std::abs(height - static_cast<int>(TileHeight(TileXY(x + 1, y)))) <= 1 && std::abs(height - static_cast<int>(TileHeight(TileXY(x, y + 1)))) <= 1, "terraform hill has invalid adjacent heights");
        }
    }
    if (setup.value("snow", false)) Tile(TileXY(8, 8)).m3() |= 16;
    if (setup.value("tunnel", false)) {
        auto cost = Command<CMD_BUILD_TUNNEL>::Do({DoCommandFlag::Execute}, TileXY(3, 8), TRANSPORT_ROAD, 0);
        Require(cost.Succeeded(), "native terraform tunnel fixture construction failed");
    }
}
inline void PrepareFixture(const Json &setup)
{
    const std::string profile = setup.at("profile");
    Require(profile == "clear" || profile == "populated", "unknown native fixture profile");
    if (profile == "populated") {
        while (LinkGraphJob::GetNumItems() > 0) {
            const size_t before = LinkGraphJob::GetNumItems();
            LinkGraphSchedule::instance.JoinNext();
            Require(LinkGraphJob::GetNumItems() < before, "populated fixture has a linkgraph job not due for native completion");
        }
    }
    AI::Uninitialize(false);
    Game::Uninitialize(false);
    for (Company *company : Company::Iterate()) company->is_ai = false;
    if (profile == "clear") {
        Require(BaseStation::GetNumItems() == 0, "clear recipe requires station-free input");
        InitializeAnimatedTiles();
        _vehicle_pool.CleanPool();
        _object_pool.CleanPool();
        _industry_pool.CleanPool();
        _town_pool.CleanPool();
        Require(Town::CanAllocateItem(), "native town pool is full");
        Town *town = new Town(TileXY(32, 32));
        town->name = "Replay Village";
        town->townnametype = SPECSTR_TOWNNAME_START;
        town->flags.Set(TownFlag::CustomGrowth);
        town->growth_rate = UINT16_MAX;
        if (setup.value("town_history", false)) {
            auto &supplied = town->GetOrCreateCargoSupplied(0);
            for (size_t i = 0; i < supplied.history.size(); ++i) {
                supplied.history[i].production = setup.value("history_fill", static_cast<uint32_t>(100 + i));
                supplied.history[i].transported = setup.value("history_fill", static_cast<uint32_t>(20 + i));
            }
            town->valid_history = setup.value("valid_history", uint64_t{0x1FFF});
            town->received[0] = {5, 9, 3, 7};
            town->road_build_months = 2;
            town->exclusive_counter = 1;
            town->exclusivity = CompanyID(0);
            town->unwanted[CompanyID(0)] = 2;
            town->ratings[CompanyID(0)] = 198;
        }
        _settings_game.economy.town_growth_rate = 0;
        _settings_game.construction.freeform_edges = false;
        for (uint32_t index = 0; index < Map::Size(); ++index) {
            const TileIndex tile(index);
            if (TileX(tile) == Map::MaxX() || TileY(tile) == Map::MaxY()) MakeVoid(tile);
            else {
                MakeClear(tile, index % 7 == 0 ? CLEAR_ROUGH : CLEAR_GRASS, setup.value("terrain_growth", false) ? index % 4 : 3);
                if (setup.value("terrain_growth", false)) SetClearCounter(tile, index % 8);
            }
            SetTileHeight(tile, 4);
        }
        _settings_game.game_creation.landscape = LandscapeType::Temperate;
        _settings_game.construction.extra_tree_placement = 0; // ETP_NO_SPREAD is private to tree_cmd.cpp.
        _settings_game.economy.type = ET_ORIGINAL;
        _settings_game.economy.inflation = false;
        _settings_game.economy.infrastructure_maintenance = false;
        _settings_game.difficulty.economy = false;
        _settings_game.difficulty.disasters = false;
        _settings_game.difficulty.max_no_competitors = 0;
        _settings_game.difficulty.subsidy_duration = 0;
        _settings_game.difficulty.industry_density = 0;
        _economy.industry_daily_change_counter = 0;
        _economy.fluct = 1;
        _new_competitor_timeout.period.value = 0;
        _new_competitor_timeout.storage.elapsed = 0;
        _new_competitor_timeout.fired = false;
        _disaster_delay = 1000;
        _cur_tileloop_tile = TileIndex(1);
    }
    if (Company::GetNumItems() == 0) Require(DoStartupNewCompany(false, CompanyID(0)) != nullptr, "native company startup failed");
    for (Company *company : Company::Iterate()) {
        company->name_1 = 0;
        company->name = "Replay Company " + std::to_string(company->index.base());
        company->president_name = "Replay President " + std::to_string(company->index.base());
        company->money = setup.value("company_money", int64_t{1000000});
        company->current_loan = setup.value("company_loan", int64_t{100000});
        if (setup.contains("company_max_loan")) company->max_loan = setup.at("company_max_loan").get<int64_t>();
        if (setup.contains("company_limit")) {
            company->clear_limit = setup.at("company_limit").get<uint32_t>();
            company->terraform_limit = company->clear_limit;
            company->tree_limit = company->clear_limit;
            company->build_object_limit = company->clear_limit;
        }
        if (setup.value("unnamed_company", false) && company->index == CompanyID(0)) {
            company->name_1 = STR_SV_UNNAMED;
            company->name.clear();
        }
    }
    _pause_mode = setup.value("paused", profile == "populated") ? PauseModes{PauseMode::Normal} : PauseModes{};
    _settings_game.construction.command_pause_level = static_cast<CommandPauseLevel>(setup.value("pause_level", uint8_t{3}));
    if (setup.contains("date")) {
        const auto &date = setup.at("date");
        const int year = date.at("year");
        const uint month = date.at("month");
        const uint day = date.at("day");
        const uint16_t fract = date.at("fract");
        TimerGameCalendar::SetDate(TimerGameCalendar::ConvertYMDToDate(TimerGameCalendar::Year(year), month, day), fract);
        TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year(year), month, day), fract);
        TimerGameEconomy::days_since_last_month = day - 1;
    }
    if (setup.contains("tick")) TimerGameTick::counter = setup.at("tick").get<uint64_t>();
    _local_company = CompanyID(0);
    _current_company = CompanyID(0);
    if (setup.contains("terraform")) PrepareTerraform(setup.at("terraform"));
}
}
#endif
