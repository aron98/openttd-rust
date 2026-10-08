// SPDX-License-Identifier: GPL-2.0-only
// Included only by the instrumented, pinned OpenTTD reference build.
#ifndef OTTD_REFERENCE_SNAPSHOT_HPP
#define OTTD_REFERENCE_SNAPSHOT_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../map_func.h"
#include "../core/random_func.hpp"
#include "../timer/timer_game_calendar.h"
#include <fstream>

#include "reference_gameplay.hpp"

extern const SaveLoadVersion SAVEGAME_VERSION;

namespace ReferenceSnapshot {
using Json = nlohmann::json;
inline Json state;
inline bool Enabled() { return std::getenv("OTTD_SNAPSHOT_PATH") != nullptr; }
inline Json Scalar(const void *address, VarType type)
{
    switch (GetVarMemType(type)) {
        case SLE_VAR_BL: return *static_cast<const bool *>(address) ? 1 : 0;
        case SLE_VAR_I8: return *static_cast<const int8_t *>(address);
        case SLE_VAR_U8: return *static_cast<const uint8_t *>(address);
        case SLE_VAR_I16: return *static_cast<const int16_t *>(address);
        case SLE_VAR_U16: return *static_cast<const uint16_t *>(address);
        case SLE_VAR_I32: return *static_cast<const int32_t *>(address);
        case SLE_VAR_U32: return *static_cast<const uint32_t *>(address);
        case SLE_VAR_I64: return *static_cast<const int64_t *>(address);
        case SLE_VAR_U64: return *static_cast<const uint64_t *>(address);
        default: throw std::runtime_error("Unsupported reference scalar");
    }
}
inline void Capture(const char *section, const void *object, SaveLoadTable table)
{
    if (!Enabled()) return;
    Json result = Json::object();
    for (const auto &field : table) {
        if (!SlIsObjectCurrentlyValid(field.version_from, field.version_to)) continue;
        const void *address = GetVariableAddress(object, field);
        switch (field.cmd) {
            case SL_VAR: result[field.name] = Scalar(address, field.conv); break;
            case SL_STDSTR: result[field.name] = *static_cast<const std::string *>(address); break;
            case SL_ARR: {
                auto values = Json::array();
                for (size_t i = 0; i < field.length; ++i) {
                    values.push_back(Scalar(static_cast<const char *>(address) + i * SlVarSize(field.conv), field.conv));
                }
                result[field.name] = std::move(values);
                break;
            }
            default: throw std::runtime_error("Unsupported reference descriptor: " + field.name);
        }
    }
    if (std::string_view(section) == "date") {
        result["random_state"] = Json::array({result.at("random_state[0]"), result.at("random_state[1]")});
        result.erase("random_state[0]"); result.erase("random_state[1]");
    }
    state[section] = std::move(result);
}
inline void Write(const char *path, const Json &value)
{
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << value.dump() << '\n';
    stream.close();
}
inline Json Primitives()
{
    Json result;
    result["rng"] = Json::array();
    for (uint32_t seed : {0U, 1U, 0xFFFFFFFFU, 305419896U}) {
        Randomizer random;
        random.SetSeed(seed);
        if (seed == 305419896U) { random.state[0] = 0xFFFFFFFFU; random.state[1] = seed; }
        Json entry = {{"seed", seed}, {"initial_state", {random.state[0], random.state[1]}}, {"steps", Json::array()}};
        for (int i = 0; i < 8; ++i) {
            auto value = random.Next();
            entry["steps"].push_back({{"limit", nullptr}, {"value", value}, {"state", {random.state[0], random.state[1]}}});
        }
        for (uint32_t limit : {0U, 1U, 65535U, 65536U, 0xFFFFFFFFU}) {
            auto value = random.Next(limit);
            entry["steps"].push_back({{"limit", limit}, {"value", value}, {"state", {random.state[0], random.state[1]}}});
        }
        result["rng"].push_back(std::move(entry));
    }
    result["calendar"] = Json::array();
    for (auto ymd : {std::array<int, 3>{0, 0, 1}, {0, 1, 28}, {0, 1, 29}, {0, 2, 1}, {1900, 1, 28}, {1900, 2, 1}, {2000, 1, 28}, {2000, 1, 29}, {2000, 2, 1}, {5000000, 11, 31}}) {
        auto date = TimerGameCalendar::ConvertYMDToDate(TimerGameCalendar::Year{ymd[0]}, ymd[1], ymd[2]);
        auto back = TimerGameCalendar::ConvertDateToYMD(date);
        result["calendar"].push_back({{"year", ymd[0]}, {"month", ymd[1]}, {"day", ymd[2]}, {"date", date.base()}, {"roundtrip", {{"year", back.year.base()}, {"month", back.month}, {"day", back.day}}}});
    }
    result["map"] = {{"width", Map::SizeX()}, {"height", Map::SizeY()}, {"coordinates", Json::array()}};
    for (uint32_t x : {0U, 1U, Map::SizeX() - 1}) {
        for (uint32_t y : {0U, 1U, Map::SizeY() - 1}) {
            auto tile = TileXY(x, y);
            result["map"]["coordinates"].push_back({{"x", x}, {"y", y}, {"tile", tile.base()}, {"roundtrip_x", TileX(tile)}, {"roundtrip_y", TileY(tile)}});
        }
    }
    return result;
}
inline void Begin()
{
    if (Enabled()) state = {{"schema_version", 1}, {"savegame_version", SAVEGAME_VERSION}, {"script_random", Json::array()}};
}
inline void Finish()
{
    if (const char *path = std::getenv("OTTD_GAMEPLAY_PROBES_PATH")) ReferenceGameplay::Run(path);
    if (!Enabled()) return;
    state["map"] = {{"width", Map::SizeX()}, {"height", Map::SizeY()}, {"tiles", Json::array()}};
    for (uint32_t i = 0; i < Map::Size(); ++i) {
        Tile t(i);
        state["map"]["tiles"].push_back({{"type", t.type()}, {"height", t.height()}, {"m1", t.m1()}, {"m2", t.m2()}, {"m3", t.m3()}, {"m4", t.m4()}, {"m5", t.m5()}, {"m6", t.m6()}, {"m7", t.m7()}, {"m8", t.m8()}});
    }
    Write(std::getenv("OTTD_SNAPSHOT_PATH"), state);
    if (const char *path = std::getenv("OTTD_PRIMITIVES_PATH")) Write(path, Primitives());
}
}
#endif
