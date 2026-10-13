// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_INDUSTRY_CACHE_SNAPSHOT_HPP
#define OTTD_REFERENCE_INDUSTRY_CACHE_SNAPSHOT_HPP
#include "../industry.h"
#include "../economy_base.h"
namespace ReferenceIndustryCache {
inline nlohmann::json Snapshot()
{
    nlohmann::json types = nlohmann::json::array();
    for (const auto &members : Industry::industries) {
        nlohmann::json ids = nlohmann::json::array();
        for (IndustryID id : members) ids.push_back(id.base());
        types.push_back(ids);
    }
    return {{"type_members", types}, {"daily_increment", _economy.industry_daily_increment}};
}
}
#endif
