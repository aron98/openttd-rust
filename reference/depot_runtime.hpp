#ifndef OTTD_REFERENCE_DEPOT_RUNTIME_HPP
#define OTTD_REFERENCE_DEPOT_RUNTIME_HPP
#include "reference_runtime_road_fixture.hpp"
#include "../depot_base.h"
#include "../tunnelbridge_map.h"
#include "../town.h"
#include "../saveload/saveload_internal.h"
#include "../core/pool_func.hpp"
#include "../3rdparty/nlohmann/json.hpp"
#include <fstream>

extern Company *DoStartupNewCompany(bool is_ai, CompanyID company);
extern RoadTypes _roadtypes_road;
extern RoadTypes _roadtypes_tram;
extern RoadTypeInfo _roadtypes[ROADTYPE_END];

namespace ReferenceDepotRuntime {
using Json = nlohmann::json;
inline Json Snapshot()
{
    Json occupied = Json::array(), road = Json::object();
    for (const Depot *depot : Depot::Iterate()) occupied.push_back(depot->index.base());
    for (const Company *company : Company::Iterate()) road[std::to_string(company->index.base())] = company->infrastructure.road;
    return {{"pool", {{"first_free", _depot_pool.first_free}, {"first_unused", _depot_pool.first_unused},
        {"items", _depot_pool.items}, {"slots", _depot_pool.data.size()}, {"occupied", occupied}}}, {"road", road}};
}
inline std::array<uint16_t, 10> Parts(Tile tile)
{
    return {tile.type(), tile.height(), tile.m1(), tile.m2(), tile.m3(), tile.m4(), tile.m5(), tile.m6(), tile.m7(), tile.m8()};
}
inline void Restore(Tile tile, const std::array<uint16_t, 10> &p)
{
    tile.type() = p[0]; tile.height() = p[1]; tile.m1() = p[2]; tile.m2() = p[3];
    tile.m3() = p[4]; tile.m4() = p[5]; tile.m5() = p[6]; tile.m6() = p[7]; tile.m7() = p[8]; tile.m8() = p[9];
}
inline void Prepare()
{
    if (Vehicle::GetNumItems() != 0 || !_grfconfig.empty() || !Company::IsValidID(CompanyID(0))) throw std::runtime_error("Depot fixture requires vehicle-free vanilla company0");
    AutoRestoreBackup company(_current_company, CompanyID(0));
    Company::Get(CompanyID(0))->money = Money{100000000};
    if (!Company::IsValidID(CompanyID(1)) && DoStartupNewCompany(false, CompanyID(1)) == nullptr) throw std::runtime_error("Depot fixture company construction failed");
    ReferenceRuntimeRoadFixture::BuildCoverageTiles();
    const TileIndex low = TileXY(20, 24), high = TileXY(22, 24);
    for (TileIndex tile : {low, high}) if (!IsTileType(tile, MP_CLEAR) || TileHeight(tile) != 4) throw std::runtime_error("Depot fixture pad is occupied");
    ReferenceRuntimeRoadFixture::RequireSuccess(Command<CMD_BUILD_ROAD_DEPOT>::Do(DoCommandFlag::Execute, low, ROADTYPE_ROAD, DIAGDIR_NE));
    if (Depot::IsValidID(DepotID(130))) throw std::runtime_error("Depot sparse identity already occupied");
    Depot *depot = new (DepotID(130)) Depot(high);
    MakeRoadDepot(high, CompanyID(1), depot->index, DIAGDIR_SW, ROADTYPE_TRAM);
    MakeDefaultName(depot);
    AfterLoadCompanyStats();
    _pause_mode = PauseMode::Normal;
}
inline Json Vectors()
{
    const TileIndex index = TileXY(8, 4);
    if (!IsNormalRoadTile(index)) throw std::runtime_error("Counter vector source is not original normal road");
    const auto saved = Parts(index);
    const Json before = Snapshot();
    std::array<RoadTypeInfo, ROADTYPE_END> types;
    for (uint8_t rt = 0; rt < ROADTYPE_END; ++rt) types[rt] = *GetRoadTypeInfo(static_cast<RoadType>(rt));
    const RoadTypes roads = _roadtypes_road, trams = _roadtypes_tram;
    for (uint8_t rt = 2; rt < ROADTYPE_END; ++rt) {
        if (AllocateRoadType(0x50520000U + rt, rt % 2 == 0 ? RTT_ROAD : RTT_TRAM) != rt) throw std::runtime_error("Counter-only native type allocation failed");
    }
    Json vectors = Json::array();
    const std::array<Owner, 4> owners = {CompanyID(0), CompanyID(1), OWNER_TOWN, OWNER_NONE};
    for (uint8_t rt = 0; rt < ROADTYPE_END; ++rt) {
        for (Owner owner : owners) {
            const RoadType type = static_cast<RoadType>(rt);
            const bool tram = GetRoadTramType(type) == RTT_TRAM;
            MakeRoadNormal(index, ROAD_ALL, tram ? INVALID_ROADTYPE : type, tram ? type : INVALID_ROADTYPE,
                TownID::Invalid(), tram ? OWNER_NONE : owner, tram ? owner : OWNER_NONE);
            AfterLoadCompanyStats();
            vectors.push_back({{"index", index.base()}, {"parts", Parts(index)}, {"runtime", Snapshot()}});
        }
    }
    for (uint8_t bits = 0; bits < 16; ++bits) {
        MakeRoadNormal(index, static_cast<RoadBits>(bits), ROADTYPE_ROAD, ROADTYPE_TRAM, TownID::Invalid(), CompanyID(0), CompanyID(1));
        SetRoadBits(index, static_cast<RoadBits>(15 - bits), RTT_TRAM);
        AfterLoadCompanyStats();
        vectors.push_back({{"index", index.base()}, {"parts", Parts(index)}, {"runtime", Snapshot()}});
    }
    Restore(index, saved);
    const TileIndex bridge = TileXY(8, 16), tunnel = TileXY(27, 12);
    const std::array<TileIndex, 10> surfaces = {TileXY(8, 6), TileXY(8, 8), TileXY(12, 8), TileXY(8, 12),
        bridge, GetOtherTunnelBridgeEnd(bridge), tunnel, GetOtherTunnelBridgeEnd(tunnel), TileXY(20, 24), TileXY(22, 24)};
    for (TileIndex tile : surfaces) {
        const auto original = Parts(tile);
        for (uint8_t mask = 0; mask < 4; ++mask) for (size_t owner = 0; owner < owners.size(); ++owner) {
            Restore(tile, original);
            SetRoadTypes(tile, (mask & 1) ? ROADTYPE_ROAD : INVALID_ROADTYPE, (mask & 2) ? ROADTYPE_TRAM : INVALID_ROADTYPE);
            SetRoadOwner(tile, RTT_ROAD, owners[owner]);
            SetRoadOwner(tile, RTT_TRAM, owners[(owner + 1) % owners.size()]);
            if (IsRoadDepotTile(tile)) SetTileOwner(tile, owners[(owner + 2) % owners.size()]);
            AfterLoadCompanyStats();
            vectors.push_back({{"index", tile.base()}, {"parts", Parts(tile)}, {"runtime", Snapshot()}});
        }
        Restore(tile, original);
    }
    for (uint8_t rt = 0; rt < ROADTYPE_END; ++rt) _roadtypes[rt] = types[rt];
    _roadtypes_road = roads;
    _roadtypes_tram = trams;
    AfterLoadCompanyStats();
    if (Snapshot() != before) throw std::runtime_error("Counter-only vector state leaked");
    return vectors;
}
inline void Observe()
{
    const char *path = std::getenv("OTTD_DEPOT_RUNTIME_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(path).good()) throw std::runtime_error("Depot runtime observation already exists");
    if (std::getenv("OTTD_DEPOT_RUNTIME_PREPARE") != nullptr) Prepare();
    Json result = {{"schema_version", 1}, {"runtime", Snapshot()}};
    if (std::getenv("OTTD_DEPOT_RUNTIME_VECTORS") != nullptr) result["counter_vectors"] = Vectors();
    std::ofstream output(path);
    output << result.dump(2) << '\n';
    if (!output) throw std::runtime_error("Depot runtime observation failed");
}
}
#endif
