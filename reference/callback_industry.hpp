// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_INDUSTRY_HPP
#define OTTD_REFERENCE_CALLBACK_INDUSTRY_HPP
#include "reference_callback_industry_state.hpp"
namespace ReferenceCallbackIndustry {
inline Json Case(const char *name, uint8_t month, int year, uint days)
{
    Json before = State(), phase;
    auto &timers = TimerManager<TimerGameEconomy>::GetTimers();
    auto original = timers;
    for (auto it = timers.begin(); it != timers.end();) {
        if ((*it)->period.trigger == TimerGameEconomy::MONTH && (*it)->period.priority == TimerGameEconomy::Priority::INDUSTRY) ++it;
        else it = timers.erase(it);
    }
    if (timers.size() != 1) throw std::runtime_error("Original industry monthly callback not found");
    TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year{year}, month, 1) - 1, 73);
    TimerGameEconomy::days_since_last_month = days - 1U;
    {
        IntervalTimer<TimerGameEconomy> observer({TimerGameEconomy::MONTH, TimerGameEconomy::Priority::NONE}, [&](uint) {
            phase = {{"month", TimerGameEconomy::month}, {"year", TimerGameEconomy::year.base()}, {"days_since_last_month", TimerGameEconomy::days_since_last_month}};
        });
        TimerManager<TimerGameEconomy>::Elapsed(1);
    }
    timers = std::move(original);
    return {{"name", name}, {"before", before}, {"operation", {{"kind", "industry_month"}, {"phase", phase}}}, {"after", State()}};
}
inline Json Run()
{
    _settings_game.economy.type = ET_ORIGINAL;
    _settings_game.economy.timekeeping_units = TKU_CALENDAR;
    _settings_game.difficulty.industry_density = ID_NORMAL;
    _current_company = CompanyID{14};
    Map::Allocate(64, 64);
    _industry_pool.CleanPool();
    Industry::industries.fill({});
    _industry_builder.wanted_inds = 0;
    Json cases = Json::array();
    cases.push_back(Case("industry_month_empty_builder", 1, 2001, 31));
    Populate();
    cases.push_back(Case("industry_month_full_history_saturation", 0, 2001, 0));
    cases.push_back(Case("industry_month_repeat", 1, 2001, 31));
    for (auto [name, month, mask] : {std::tuple{"industry_month_partial_quarter", 3, (1ULL << 22) - 2},
        std::tuple{"industry_month_partial_year", 0, (1ULL << 39) - 2},
        std::tuple{"industry_month_sparse_mask", 0, (1ULL << 24) | (1ULL << 63)},
        std::tuple{"industry_month_zero_mask", 2, 0ULL}}) {
        Populate();
        for (Industry *i : Industry::Iterate()) i->valid_history = mask;
        cases.push_back(Case(name, uint8_t(month), 2001, 30));
    }
    _settings_game.difficulty.industry_density = ID_FUND_ONLY;
    _industry_builder.wanted_inds = 0;
    cases.push_back(Case("industry_month_fund_only", 3, 2001, 1));
    _settings_game.difficulty.industry_density = ID_NORMAL;
    _industry_builder.wanted_inds = 100U << 16;
    cases.push_back(Case("industry_month_builder_behind", 4, 2001, 30));
    _industry_builder.wanted_inds = 4U << 16;
    cases.push_back(Case("industry_month_builder_equal_threshold", 5, 2001, 31));
    Map::Allocate(128, 64);
    _industry_builder.wanted_inds = 0;
    cases.push_back(Case("industry_month_rectangular_scale", 6, 2001, UINT32_MAX));
    Map::Allocate(2048, 2048);
    _industry_builder.wanted_inds = 102U << 16;
    cases.push_back(Case("industry_month_large_map_backlog_cap", 7, 2001, 31));
    Map::Allocate(64, 64);
    Populate();
    _industry_builder.wanted_inds = UINT32_MAX;
    cases.push_back(Case("industry_month_year_before_rewind", 0, 5000001, 31));
    cases.push_back(Case("industry_month_after_rewind", 1, 5000000, 31));
    return cases;
}
}
#endif
