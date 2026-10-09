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

extern TimeoutTimer<TimerGameTick> _new_competitor_timeout;
extern uint16_t _disaster_delay;
extern TileIndex _cur_tileloop_tile;
Company *DoStartupNewCompany(bool is_ai, CompanyID company);

namespace ReferenceReplay {
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
        _settings_game.economy.town_growth_rate = 0;
        _settings_game.construction.freeform_edges = false;
        for (uint32_t index = 0; index < Map::Size(); ++index) {
            const TileIndex tile(index);
            if (TileX(tile) == Map::MaxX() || TileY(tile) == Map::MaxY()) MakeVoid(tile);
            else MakeClear(tile, index % 7 == 0 ? CLEAR_ROUGH : CLEAR_GRASS, 3);
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
        company->money = 1000000;
        company->current_loan = 100000;
    }
    _pause_mode = setup.value("paused", profile == "populated") ? PauseModes{PauseMode::Normal} : PauseModes{};
    _settings_game.construction.command_pause_level = CommandPauseLevel::AllActions;
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
}
}
#endif
