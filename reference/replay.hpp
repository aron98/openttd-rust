// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_HPP
#define OTTD_REFERENCE_REPLAY_HPP
#include "saveload.h"
#include "saveload_internal.h"
#include "reference_world.hpp"
#include "reference_world_derived.hpp"
#include "../core/random_func.hpp"
#include "../timer/timer_game_calendar.h"
#include "../timer/timer_game_economy.h"
#include "../timer/timer_game_tick.h"
#include "../3rdparty/nlohmann/json.hpp"
#include <filesystem>

namespace ReferenceReplay {
using Json = nlohmann::json;
using ReferenceWorld::Require;
inline Json *receipt = nullptr;
inline Json metadata;
}
#include "reference_replay_cost.hpp"
#include "reference_replay_commands.hpp"
#include "reference_replay_fixture.hpp"

namespace ReferenceReplay {
inline Json Runtime()
{
    return {{"tick", TimerGameTick::counter}, {"pause", _pause_mode.base()},
        {"calendar_date", TimerGameCalendar::date.base()}, {"calendar_fract", TimerGameCalendar::date_fract},
        {"calendar_year", TimerGameCalendar::year.base()}, {"calendar_month", TimerGameCalendar::month},
        {"economy_date", TimerGameEconomy::date.base()}, {"economy_fract", TimerGameEconomy::date_fract},
        {"economy_year", TimerGameEconomy::year.base()}, {"economy_month", TimerGameEconomy::month},
        {"random", {_random.state[0], _random.state[1]}},
        {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"current_company", _current_company.base()}};
}
inline void Write(const std::filesystem::path &path, const Json &value)
{
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << value.dump() << '\n';
    stream.close();
}
inline Json Checkpoint(const std::filesystem::path &directory, const std::string &label)
{
    Require(!label.empty() && label.find_first_not_of("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_") == std::string::npos, "invalid checkpoint label");
    const std::string stem = (directory / label).string();
    Require(!std::filesystem::exists(stem + ".sav"), "stale replay checkpoint");
    setenv("OTTD_WORLD_PATH", (stem + ".world.json").c_str(), 1);
    setenv("OTTD_WORLD_SCHEMA_PATH", (stem + ".schema.json").c_str(), 1);
    setenv("OTTD_WORLD_DERIVED_PATH", (stem + ".derived.json").c_str(), 1);
    const Json before = Runtime();
    Require(SaveOrLoad(stem + ".sav", SLO_SAVE, DFT_GAME_FILE, NO_DIRECTORY, false) == SL_OK, "native checkpoint save failed");
    ReferenceWorld::AfterLoad();
    const Json after = Runtime();
    Write(stem + ".runtime.json", {{"before_save", before}, {"after_save", after}});
    return {{"label", label}, {"runtime", after}};
}
inline bool Run()
{
    const char *path = std::getenv("OTTD_REPLAY_PATH");
    static bool active = false;
    static bool done = false;
    if (path == nullptr || active || done || _game_mode != GM_NORMAL) return false;
    active = true;
    std::ifstream input(path);
    Require(input.good(), "cannot open native replay input");
    const Json protocol = Json::parse(input);
    Require(protocol.at("schema_version") == 1, "unsupported replay schema");
    const char *output = std::getenv("OTTD_REPLAY_OUTPUT");
    Require(output != nullptr, "native replay output directory required");
    const std::filesystem::path directory(output);
    std::filesystem::create_directories(directory);
    Require(!std::filesystem::exists(directory / "results.json"), "stale native replay results");
    Json results = {{"schema_version", 1}, {"actions", Json::array()}, {"checkpoints", Json::array()}};
    if (protocol.contains("fixture")) PrepareFixture(protocol.at("fixture"));
    results["checkpoints"].push_back(Checkpoint(directory, "initial"));
    const Json &actions = protocol.at("actions");
    Require(actions.is_array() && actions.size() <= 10000, "invalid replay action collection");
    uint64_t previous = 0;
    bool first = true;
    uint64_t total_ticks = 0;
    for (const auto &action : actions) {
        const uint64_t ordinal = action.at("ordinal");
        Require(first || ordinal > previous, "non-increasing replay ordinal");
        first = false;
        previous = ordinal;
        const std::string op = action.at("op");
        Json observation = {{"ordinal", ordinal}, {"op", op}, {"before", Runtime()}};
        if (op == "command") {
            observation["receipt"] = Execute(action.at("request"));
            observation["native_metadata"] = metadata;
        } else if (op == "tick") {
            const uint32_t count = action.at("count");
            total_ticks += count;
            Require(total_ticks <= 100000, "native replay tick limit exceeded");
            for (uint32_t i = 0; i < count; ++i) StateGameLoop();
        } else if (op == "checkpoint") {
            results["checkpoints"].push_back(Checkpoint(directory, action.at("label")));
        } else {
            throw std::runtime_error("unknown native replay operation: " + op);
        }
        observation["after"] = Runtime();
        results["actions"].push_back(std::move(observation));
    }
    results["checkpoints"].push_back(Checkpoint(directory, "final"));
    Write(directory / "results.json", results);
    unsetenv("OTTD_WORLD_PATH");
    unsetenv("OTTD_WORLD_SCHEMA_PATH");
    unsetenv("OTTD_WORLD_DERIVED_PATH");
    _settings_client.gui.autosave_on_exit = false;
    done = true;
    active = false;
    _exit_game = true;
    return true;
}
}
#endif
