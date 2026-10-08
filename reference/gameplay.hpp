// SPDX-License-Identifier: GPL-2.0-only
// Probe-only process: ordinary callbacks are disabled; native timer and tile routines are unchanged.
#ifndef OTTD_REFERENCE_GAMEPLAY_HPP
#define OTTD_REFERENCE_GAMEPLAY_HPP
#include "../clear_map.h"
#include "../void_map.h"
#include "../landscape.h"
#include "../openttd.h"
#include "../newgrf_generic.h"
#include "../timer/timer.h"
#include "../timer/timer_game_economy.h"
#include "../timer/timer_game_tick.h"

extern TileIndex _cur_tileloop_tile;
namespace ReferenceGameplay {
using Json = nlohmann::json;
inline Json events;
inline Json Clock()
{
    return {{"date", TimerGameCalendar::date.base()}, {"date_fract", TimerGameCalendar::date_fract},
        {"calendar_sub_date_fract", TimerGameCalendar::sub_date_fract}, {"calendar_year", TimerGameCalendar::year.base()}, {"calendar_month", TimerGameCalendar::month},
        {"economy_date", TimerGameEconomy::date.base()}, {"economy_date_fract", TimerGameEconomy::date_fract},
        {"economy_year", TimerGameEconomy::year.base()}, {"economy_month", TimerGameEconomy::month},
        {"days_since_last_month", TimerGameEconomy::days_since_last_month}, {"tick_counter", TimerGameTick::counter}};
}
inline Json Context(bool paused = false)
{
    return {{"landscape", "temperate"}, {"mode", "normal"}, {"ambient_callbacks", false}, {"paused", paused},
        {"timekeeping_units", static_cast<int>(_settings_game.economy.timekeeping_units)},
        {"minutes_per_calendar_year", _settings_game.economy.minutes_per_calendar_year}};
}
inline void ResetClock(int year, int month, int day, uint16_t fraction, int units, uint16_t minutes, uint16_t subfraction = 0)
{
    _game_mode = GM_NORMAL;
    _settings_game.economy.timekeeping_units = static_cast<TimekeepingUnits>(units);
    _settings_game.economy.minutes_per_calendar_year = minutes;
    TimerGameCalendar::SetDate(TimerGameCalendar::ConvertYMDToDate(TimerGameCalendar::Year{year}, month, day), fraction);
    TimerGameCalendar::sub_date_fract = subfraction;
    TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year{year}, month, day), fraction);
    TimerGameEconomy::days_since_last_month = day - 1;
    TimerGameTick::counter = 0;
    _random.SetSeed(305419896U);
}
inline bool Advance(bool paused)
{
    events = Json::array();
    if (paused) return false;
    bool progressed = TimerManager<TimerGameCalendar>::Elapsed(1);
    TimerManager<TimerGameEconomy>::Elapsed(1);
    TimerManager<TimerGameTick>::Elapsed(1);
    return progressed;
}
inline Json RawMap()
{
    Json map = {{"width", Map::SizeX()}, {"height", Map::SizeY()}, {"tiles", Json::array()}};
    for (uint32_t i = 0; i < Map::Size(); ++i) {
        Tile t(i);
        map["tiles"].push_back({{"type", t.type()}, {"height", t.height()}, {"m1", t.m1()}, {"m2", t.m2()},
            {"m3", t.m3()}, {"m4", t.m4()}, {"m5", t.m5()}, {"m6", t.m6()}, {"m7", t.m7()}, {"m8", t.m8()}});
    }
    return map;
}
inline Json State()
{
    return {{"map", RawMap()}, {"clock", Clock()}, {"cur_tileloop_tile", _cur_tileloop_tile.base()},
        {"random_state", {_random.state[0], _random.state[1]}}};
}
inline Json Document(const Json &records, bool paused = false)
{
    return {{"schema_version", 1}, {"context", Context(paused)}, {"state", State()}, {"events", records}};
}
inline void Run(const char *path)
{
    if (HasGrfMiscBit(GrfMiscBit::AmbientSoundCallback)) throw std::runtime_error("Gameplay oracle requires no ambient NewGRF callbacks");
    TimerManager<TimerGameCalendar>::GetTimers().clear();
    TimerManager<TimerGameEconomy>::GetTimers().clear();
    TimerManager<TimerGameTick>::GetTimers().clear();
    std::vector<std::unique_ptr<IntervalTimer<TimerGameCalendar>>> calendar_timers;
    std::vector<std::unique_ptr<IntervalTimer<TimerGameEconomy>>> economy_timers;
    for (auto [trigger, name] : {std::pair{TimerGameCalendar::DAY, "calendar_day"}, {TimerGameCalendar::MONTH, "calendar_month"}, {TimerGameCalendar::YEAR, "calendar_year"}}) {
        calendar_timers.push_back(std::make_unique<IntervalTimer<TimerGameCalendar>>(TimerGameCalendar::TPeriod{trigger, TimerGameCalendar::NONE}, [name](uint) { events.push_back(name); }));
    }
    for (auto [trigger, name] : {std::pair{TimerGameEconomy::DAY, "economy_day"}, {TimerGameEconomy::WEEK, "economy_week"}, {TimerGameEconomy::MONTH, "economy_month"}, {TimerGameEconomy::QUARTER, "economy_quarter"}, {TimerGameEconomy::YEAR, "economy_year"}}) {
        economy_timers.push_back(std::make_unique<IntervalTimer<TimerGameEconomy>>(TimerGameEconomy::TPeriod{trigger, TimerGameEconomy::NONE}, [name](uint) { events.push_back(name); }));
    }
    Json output = {{"schema_version", 1}, {"clock_cases", Json::array()}, {"landscape_cases", Json::array()}};
    auto clock_case = [&](const char *name, int year, int month, int day, uint16_t fraction, int units, uint16_t minutes, uint16_t subfraction, uint ticks, bool paused = false, bool wrap = false) {
        ResetClock(year, month, day, fraction, units, minutes, subfraction);
        if (wrap) TimerGameTick::counter = UINT64_MAX;
        Json entry = {{"name", name}, {"context", Context(paused)}, {"before", Clock()}, {"steps", Json::array()}, {"random_before", {_random.state[0], _random.state[1]}}};
        for (uint i = 0; i < ticks; ++i) {
            bool progressed = Advance(paused);
            entry["steps"].push_back({{"after", Clock()}, {"calendar_progressed", progressed}, {"events", events}});
        }
        entry["random_after"] = {_random.state[0], _random.state[1]};
        output["clock_cases"].push_back(std::move(entry));
    };
    clock_case("ordinary", 1950, 0, 1, 0, 0, 12, 0, 75);
    clock_case("leap_february", 2000, 1, 28, 73, 0, 12, 0, 76);
    clock_case("century_february", 2100, 1, 28, 73, 0, 12, 0, 2);
    clock_case("quarter", 2000, 2, 31, 73, 0, 12, 0, 2);
    clock_case("year", 1999, 11, 31, 73, 0, 12, 0, 2);
    clock_case("frozen", 2000, 0, 30, 73, 1, 0, 17, 75);
    clock_case("slow13", 2000, 0, 30, 73, 1, 13, 73, 4);
    clock_case("slow24", 2000, 0, 30, 73, 1, 24, 73, 4);
    clock_case("slow10080", 2000, 0, 30, 73, 1, 10080, 62090, 4);
    clock_case("wallclock_quarter", 2000, 2, 30, 73, 1, 12, 0, 2);
    clock_case("wallclock_year", 2000, 11, 30, 73, 1, 12, 0, 2);
    clock_case("paused", 2000, 0, 1, 73, 0, 12, 7, 2, true);
    clock_case("tick_wrap", 2000, 0, 1, 0, 0, 12, 0, 2, false, true);
    clock_case("maximum_calendar", 5000000, 11, 31, 73, 0, 12, 0, 76);
    clock_case("maximum_wallclock", 5000000, 11, 30, 73, 1, 12, 0, 76);

    for (uint scenario = 0; scenario < 4; ++scenario) {
        uint width = scenario == 1 ? 128 : 64;
        bool paused = scenario == 2;
        Map::Allocate(width, 64);
        _settings_game.game_creation.landscape = LandscapeType::Temperate;
        _settings_game.construction.freeform_edges = false;
        ResetClock(2000, 1, 28, 73, 0, 12);
        _cur_tileloop_tile = TileIndex{1};
        if (scenario == 3) TimerGameTick::counter = UINT64_MAX;
        for (uint i = 0; i < Map::Size(); ++i) {
            Tile t(i);
            t.type() = 0;
            if (TileX(t) == Map::MaxX() || TileY(t) == Map::MaxY()) { MakeVoid(t); continue; }
            MakeClear(t, i % 6 == 4 ? CLEAR_ROUGH : i % 6 == 5 ? CLEAR_ROCKS : CLEAR_GRASS, i % 4);
            SetClearCounter(t, (i / 4) % 8);
            t.type() |= i % 4;
            t.height() = i % 16;
            t.m3() = i % 16;
            t.m6() = (i * 13) % 256;
            t.m2() = i % 65536;
            t.m4() = i % 256;
            t.m7() = (i * 3) % 256;
            t.m8() = (i * 7) % 65536;
        }
        Json records = Json::array();
        Json entry = {{"name", scenario == 2 ? "paused" : scenario == 3 ? "tick_wrap" : "temperate_" + std::to_string(width) + "x64"}, {"before", Document(records, paused)}, {"checkpoints", Json::array()}};
        uint previous = 0;
        auto checkpoints = scenario < 2 ? std::vector<uint>{0, 1, 255, 256, 257, 2048, 6144} : std::vector<uint>{0, 1, 2, 256};
        for (uint checkpoint : checkpoints) {
            for (uint tick = previous; tick < checkpoint; ++tick) {
                bool progressed = Advance(paused);
                if (!paused) RunTileLoop();
                records.push_back({{"tick_counter", TimerGameTick::counter}, {"calendar_progressed", progressed}, {"events", events}});
            }
            entry["checkpoints"].push_back({{"ticks", checkpoint}, {"after", Document(records, paused)}});
            previous = checkpoint;
        }
        output["landscape_cases"].push_back(std::move(entry));
    }
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << output.dump() << '\n';
    stream.close();
    std::_Exit(EXIT_SUCCESS);
}
}
#endif
