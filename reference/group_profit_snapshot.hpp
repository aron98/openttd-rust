// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GROUP_PROFIT_SNAPSHOT_HPP
#define OTTD_REFERENCE_GROUP_PROFIT_SNAPSHOT_HPP
#include "../company_base.h"
#include "../group.h"
#include "../vehicle_base.h"
#include "../settings_type.h"
#include "../network/network.h"
#include "../news_gui.h"
#include "../timer/timer_game_calendar.h"
#include "../timer/timer_game_economy.h"
#include "../timer/timer_game_tick.h"
#include "../3rdparty/nlohmann/json.hpp"

namespace ReferenceGroupProfit {
using Json = nlohmann::json;
inline bool Enabled() { return std::getenv("OTTD_VEHICLE_YEAR_TRACE") != nullptr; }
inline Json Statistics(const GroupStatistics &statistics)
{
    Json engines = Json::array();
    for (const auto &[engine, count] : statistics.num_engines) engines.push_back({engine.base(), count});
    return {{"profit_last_year", static_cast<int64_t>(statistics.profit_last_year)},
        {"profit_last_year_min_age", static_cast<int64_t>(statistics.profit_last_year_min_age)},
        {"num_vehicle_min_age", statistics.num_vehicle_min_age}, {"num_vehicle", statistics.num_vehicle},
        {"num_engines", engines}, {"autoreplace_defined", statistics.autoreplace_defined},
        {"autoreplace_finished", statistics.autoreplace_finished}};
}
inline Json Snapshot()
{
    Json companies = Json::array(), groups = Json::array(), vehicles = Json::array();
    for (const Company *company : Company::Iterate()) {
        Json all = Json::array(), defaults = Json::array(), expenses = Json::array();
        for (const auto &statistics : company->group_all) all.push_back(Statistics(statistics));
        for (const auto &statistics : company->group_default) defaults.push_back(Statistics(statistics));
        for (const auto &year : company->yearly_expenses) {
            Json values = Json::array();
            for (const Money value : year) values.push_back(static_cast<int64_t>(value));
            expenses.push_back(values);
        }
        companies.push_back({{"id", company->index.base()}, {"is_ai", company->is_ai},
            {"all", all}, {"default", defaults}, {"yearly_expenses", expenses}});
    }
    for (const Group *group : Group::Iterate()) {
        groups.push_back({{"id", group->index.base()}, {"owner", group->owner.base()},
            {"type", group->vehicle_type}, {"parent", group->parent.base()},
            {"statistics", Statistics(group->statistics)}});
    }
    for (const Vehicle *vehicle : Vehicle::Iterate()) {
        vehicles.push_back({{"id", vehicle->index.base()}, {"owner", vehicle->owner.base()},
            {"type", vehicle->type}, {"group", vehicle->group_id.base()},
            {"primary", vehicle->IsPrimaryVehicle()}, {"economy_age", vehicle->economy_age.base()},
            {"profit_this_year", static_cast<int64_t>(vehicle->profit_this_year)},
            {"profit_last_year", static_cast<int64_t>(vehicle->profit_last_year)}});
    }
    return {{"schema_version", 1}, {"companies", companies}, {"groups", groups}, {"vehicles", vehicles},
        {"news_queue", GetNews().size()},
        {"context", {{"current_company", _current_company.base()}, {"local_company", _local_company.base()},
            {"networking", _networking}, {"network_server", _network_server},
            {"vehicle_income_warn", _settings_client.gui.vehicle_income_warn},
            {"old_vehicle_warn", _settings_client.gui.old_vehicle_warn}}},
        {"clock", {{"tick", TimerGameTick::counter}, {"economy_date", TimerGameEconomy::date.base()},
            {"economy_fract", TimerGameEconomy::date_fract}, {"economy_year", TimerGameEconomy::year.base()},
            {"calendar_date", TimerGameCalendar::date.base()}, {"calendar_fract", TimerGameCalendar::date_fract}}}};
}
}
#endif
