// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_COMPANY_RATING_SNAPSHOT_HPP
#define OTTD_REFERENCE_COMPANY_RATING_SNAPSHOT_HPP
#include "reference_group_profit_snapshot.hpp"
#include "../economy_func.h"

namespace ReferenceCompanyRating {
using Json = nlohmann::json;
inline bool Enabled() { return std::getenv("OTTD_COMPANY_RATING_OBSERVE") != nullptr; }
inline Json Economy(const CompanyEconomyEntry &entry)
{
    Json delivered = Json::array();
    for (const auto amount : entry.delivered_cargo) delivered.push_back(amount);
    return {{"income", static_cast<int64_t>(entry.income)},
        {"expenses", static_cast<int64_t>(entry.expenses)}, {"delivered_cargo", delivered},
        {"performance_history", entry.performance_history},
        {"company_value", static_cast<int64_t>(entry.company_value)}};
}
inline Json Snapshot()
{
    Json scores = Json::array(), info = Json::array(), companies = Json::array();
    for (uint8_t owner = 0; owner < MAX_COMPANIES; ++owner) {
        Json parts = Json::array();
        for (const int64_t part : _score_part[CompanyID{owner}]) parts.push_back(part);
        scores.push_back({{"owner", owner}, {"parts", parts}});
    }
    for (ScoreID id = SCORE_BEGIN; id < SCORE_END; ++id) {
        info.push_back({{"id", id}, {"needed", _score_info[id].needed}, {"weight", _score_info[id].score}});
    }
    for (const Company *company : Company::Iterate()) {
        Json history = Json::array();
        for (const auto &entry : company->old_economy) history.push_back(Economy(entry));
        companies.push_back({{"id", company->index.base()}, {"valid_history", company->num_valid_stat_ent},
            {"block_preview", company->block_preview}, {"money", static_cast<int64_t>(company->money)},
            {"loan", static_cast<int64_t>(company->current_loan)}, {"hq_tile", company->location_of_HQ.base()},
            {"current", Economy(company->cur_economy)}, {"history", history}});
    }
    return {{"schema_version", 1}, {"economy_month", TimerGameEconomy::month}, {"scores", scores}, {"score_info", info},
        {"companies", companies}, {"runtime", ReferenceGroupProfit::Snapshot()}};
}
}
#endif
