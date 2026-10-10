#ifndef OTTD_REFERENCE_TREE_FIXTURE_HPP
#define OTTD_REFERENCE_TREE_FIXTURE_HPP
#include "../tree_map.h"
#include "../town.h"
#include "../bridge_map.h"
#include "../cheat_type.h"

namespace ReferenceReplay {
inline uint32_t TreeFixtureUnsigned(const Json &value, uint32_t maximum)
{
    Require(value.is_number_unsigned() && value.get<uint64_t>() <= maximum, "tree fixture unsigned field out of range");
    return value.get<uint32_t>();
}
inline int16_t TreeFixtureRating(const Json &value)
{
    Require(value.is_number_integer(), "tree fixture rating must be an integer");
    if (value.is_number_unsigned()) Require(value.get<uint64_t>() <= INT16_MAX, "tree fixture rating out of range");
    else Require(value.get<int64_t>() >= INT16_MIN && value.get<int64_t>() <= INT16_MAX, "tree fixture rating out of range");
    return value.get<int16_t>();
}
inline void PrepareTrees(const Json &setup)
{
    const auto &trees = setup.at("tiles");
    const auto &towns = setup.at("towns");
    Require(trees.is_array() && trees.size() <= 64, "tree fixture tile bound");
    Require(towns.is_array() && towns.size() <= 16, "tree fixture town bound");
    for (const auto &entry : trees) {
        const TileIndex tile(TreeFixtureUnsigned(entry.at("tile"), Map::Size() - 1));
        const uint type = TreeFixtureUnsigned(entry.at("type"), TREE_TOYLAND + TREE_COUNT_TOYLAND - 1);
        const uint count = TreeFixtureUnsigned(entry.at("count"), 4);
        const uint ground = TreeFixtureUnsigned(entry.at("ground"), TREE_GROUND_ROUGH_SNOW);
        Require(IsTileType(tile, MP_CLEAR), "tree fixture needs clear tile");
        Require(!IsBridgeAbove(tile), "tree fixture bridge boundary");
        Require(count >= 1, "tree fixture needs at least one tree");
        MakeTree(tile, static_cast<TreeType>(type), count - 1, TreeGrowthStage::Grown, static_cast<TreeGround>(ground), 3);
    }
    for (const auto &entry : towns) {
        Town *town = Town::GetIfValid(TownID(static_cast<uint16_t>(TreeFixtureUnsigned(entry.at("id"), UINT16_MAX))));
        const TileIndex centre(TreeFixtureUnsigned(entry.at("tile"), Map::Size() - 1));
        Require(town != nullptr, "tree fixture needs existing town");
        town->xy = centre;
        town->ratings[CompanyID(0)] = TreeFixtureRating(entry.at("rating"));
        town->have_ratings.Reset(CompanyID(0));
        if (entry.at("have_rating").get<bool>()) town->have_ratings.Set(CompanyID(0));
    }
    _settings_game.economy.dist_local_authority = static_cast<uint8_t>(TreeFixtureUnsigned(setup.at("threshold"), UINT8_MAX));
    _cheats.magic_bulldozer.value = setup.at("magic_bulldozer").get<bool>();
    RebuildTownKdtree();
}
}
#endif
