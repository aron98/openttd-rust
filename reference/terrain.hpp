// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_TERRAIN_HPP
#define OTTD_REFERENCE_TERRAIN_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../landscape.h"
#include "../slope_func.h"
#include "../tile_map.h"
#include "../settings_type.h"
#include <fstream>

namespace ReferenceTerrain {
inline void Observe()
{
    const char *path = std::getenv("OTTD_TERRAIN_PROBES_PATH");
    if (path == nullptr) return;
    using Json = nlohmann::json;
    Json result = {{"schema_version", 1}, {"slopes", Json::array()}, {"foundations", Json::array()}};
    const std::array<uint8_t, 20> bases = {0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,23,27,29,30};
    for (uint8_t base : bases) {
        for (int half = -1; half < 4; ++half) {
            Slope slope = static_cast<Slope>(base);
            if (half >= 0) slope = HalftileSlope(slope, static_cast<Corner>(half));
            Json row = {{"slope", static_cast<uint8_t>(slope)}, {"pixels", Json::array()}, {"edges", Json::array()}};
            for (int y = 0; y < 16; ++y) for (int x = 0; x < 16; ++x) row["pixels"].push_back(GetPartialPixelZ(x, y, slope));
            for (int edge = 0; edge < 4; ++edge) {
                int near = 0, far = 0;
                GetSlopePixelZOnEdge(slope, static_cast<DiagDirection>(edge), near, far);
                row["edges"].push_back({near, far});
            }
            if (half < 0) {
                row["corners"] = Json::array();
                for (int corner = 0; corner < 4; ++corner) row["corners"].push_back(GetSlopeZInCorner(slope, static_cast<Corner>(corner)));
            }
            result["slopes"].push_back(std::move(row));
        }
        for (int f = FOUNDATION_NONE; f <= FOUNDATION_RAIL_N; ++f) {
            Slope slope = static_cast<Slope>(base);
            if (f >= FOUNDATION_INCLINED_X && f <= FOUNDATION_STEEP_BOTH && !HasSlopeHighestCorner(slope)) continue;
            uint dz = ApplyFoundationToSlope(static_cast<Foundation>(f), slope);
            result["foundations"].push_back({{"slope", base}, {"foundation", f}, {"result", {static_cast<uint8_t>(slope), dz}}});
        }
    }
    result["map"] = {{"width", Map::SizeX()}, {"height", Map::SizeY()},
        {"freeform_edges", _settings_game.construction.freeform_edges}, {"tiles", Json::array()}};
    for (uint32_t i = 0; i < Map::Size(); ++i) {
        TileIndex tile(i);
        uint x = TileX(tile), y = TileY(tile);
        uint x2 = std::min(x + 1, Map::MaxX()), y2 = std::min(y + 1, Map::MaxY());
        auto [slope, height] = GetTileSlopeZ(tile);
        result["map"]["tiles"].push_back({{"tile", i}, {"corners", {TileHeight(tile), TileHeight(TileXY(x2, y)), TileHeight(TileXY(x, y2)), TileHeight(TileXY(x2, y2))}}, {"result", {static_cast<uint8_t>(slope), height}}});
    }
    std::ofstream output(path);
    if (!output) throw std::runtime_error("cannot open terrain observation");
    output << result.dump() << '\n';
    if (!output) throw std::runtime_error("cannot write terrain observation");
}
}
#endif
