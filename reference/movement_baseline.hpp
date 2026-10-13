// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_BASELINE_HPP
#define OTTD_REFERENCE_MOVEMENT_BASELINE_HPP
#include "3rdparty/nlohmann/json.hpp"
#include "fileio_func.h"
#include "newgrf_config.h"
#include "openttd.h"
#include "saveload/reference_movement_forbidden.hpp"
#include <filesystem>
#include <fstream>
namespace ReferenceMovementBaseline {
using Json = nlohmann::json;
inline bool armed = false, consumed = false, completed = false;
inline uint64_t arms = 0, loads = 0, ignored_menu_arms = 0, ignored_loads = 0;
inline Json selected = Json::array();
inline bool Enabled()
{
    return std::getenv("OTTD_MOVEMENT_OBSERVE") != nullptr;
}
[[noreturn]] inline void Refuse(const char *message)
{
    throw std::runtime_error(message);
}
inline void Arm()
{
    if (!Enabled()) return;
    ReferenceMovement::ValidateEnvironment();
    if (_game_mode == GM_MENU) { ++ignored_menu_arms; return; }
    if (armed || consumed || completed) Refuse("movement baseline duplicate save-load arm");
    if (std::getenv("OTTD_REPLAY_OUTPUT") == nullptr) Refuse("movement baseline output missing");
    armed = true;
    ++arms;
}
inline Json Selection()
{
    Json rows = Json::array();
    for (const auto &config : _grfconfig) {
        const std::string path = FioFindFullPath(BASESET_DIR, config->filename);
        if (path.empty() || !std::filesystem::is_regular_file(path)) Refuse("movement baseline source unavailable");
        rows.push_back({{"ordinal", rows.size()}, {"path", path}, {"filename", config->filename},
            {"grfid", config->ident.grfid}, {"parameters", config->param}, {"flags", config->flags.base()},
            {"bytes", std::filesystem::file_size(path)}});
    }
    return rows;
}
inline void Consume(uint num_baseset)
{
    if (!Enabled()) return;
    if (!armed) { ++ignored_loads; return; }
    ReferenceMovement::ValidateEnvironment();
    if (consumed || completed || num_baseset != 2 || _grfconfig.size() != 2) Refuse("movement baseline consumed roster invalid");
    armed = false;
    consumed = true;
    ++loads;
    selected = Selection();
}
inline void Finish()
{
    if (!Enabled() || _game_mode == GM_MENU) return;
    if (armed || !consumed || completed || arms != 1 || loads != 1) Refuse("movement baseline save-load lifecycle invalid");
    const std::filesystem::path directory(std::getenv("OTTD_REPLAY_OUTPUT"));
    if (!std::filesystem::is_directory(directory)) Refuse("movement baseline output directory missing");
    const auto path = directory / "baseline.json";
    if (std::filesystem::exists(path)) Refuse("movement baseline stale receipt");
    const Json receipt = {{"schema_version", 1}, {"scope", "actual-save-load-baseset-selection"},
        {"arm_seam", "AfterLoadGame.before.GfxLoadSprites"}, {"consume_seam", "LoadNewGRF.entry"},
        {"arms", arms}, {"loads", loads}, {"ignored_menu_arms", ignored_menu_arms}, {"ignored_loads", ignored_loads},
        {"ordered_sources", selected}};
    std::ofstream stream(path, std::ios::binary);
    stream << receipt.dump(2) << '\n';
    stream.flush();
    if (!stream.good()) Refuse("movement baseline receipt write failure");
    completed = true;
}
inline void RequireComplete()
{
    if (!Enabled() || !completed || armed || !consumed || arms != 1 || loads != 1) Refuse("movement baseline receipt missing or duplicate");
}
}
#endif
