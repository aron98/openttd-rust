// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_HOUSE_CACHE_HPP
#define OTTD_REFERENCE_HOUSE_CACHE_HPP
#include "../house.h"
#include "../town.h"
#include "../newgrf_house.h"
std::span<const uint> ReferenceHouseGlobalClassCounts();
namespace ReferenceHouseCache {
inline nlohmann::json Snapshot()
{
    using Json = nlohmann::json;
    Json towns = Json::array();
    for (const Town *town : Town::Iterate()) {
        const auto &cache = town->cache;
        towns.push_back({{"id", town->index.base()}, {"population", cache.population},
            {"num_houses", cache.num_houses}, {"zone_radii", cache.squared_town_zone_radius},
            {"id_count", cache.building_counts.id_count}, {"class_count", cache.building_counts.class_count}});
    }
    Json counts = Json::array(), classes = Json::array();
    for (uint count : GetBuildingHouseIDCounts()) counts.push_back(count);
    for (uint count : ReferenceHouseGlobalClassCounts()) classes.push_back(count);
    return {{"towns", towns}, {"global_id_count", counts}, {"global_class_count", classes}};
}
}
#endif
