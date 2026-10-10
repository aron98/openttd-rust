// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_LOAD_CONTEXT_HPP
#define OTTD_REFERENCE_GRF_LOAD_CONTEXT_HPP
namespace ReferenceGrfContext {
using Json = nlohmann::json;
inline bool active = false, used = false;
inline std::string output;
inline Json original, prepared, baseline_globals, stages = Json::array();
[[noreturn]] inline void HostError(std::string_view message)
{
    fmt::print(stderr, "GRF context observer host refusal: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
inline Json Snapshot()
{
    return {{"calendar_date", TimerGameCalendar::date.base()}, {"calendar_year", TimerGameCalendar::year.base()},
        {"calendar_fraction", TimerGameCalendar::date_fract}, {"economy_date", TimerGameEconomy::date.base()},
        {"economy_year", TimerGameEconomy::year.base()}, {"economy_fraction", TimerGameEconomy::date_fract},
        {"tick", TimerGameTick::counter}, {"display", _display_opt}, {"networking", _networking},
        {"random", {_random.state[0], _random.state[1]}}, {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"game_mode", _game_mode}, {"timekeeping_units", _settings_game.economy.timekeeping_units},
        {"starting_year", _settings_game.game_creation.starting_year.base()}, {"climate", _settings_game.game_creation.landscape},
        {"road_side", _settings_game.vehicle.road_side}, {"disable_elrails", _settings_game.vehicle.disable_elrails},
        {"height_limit", _settings_game.construction.map_height_limit}, {"snowline", _settings_game.game_creation.snow_line_height},
        {"generation_seed", _settings_game.game_creation.generation_seed}, {"freight_trains", _settings_game.vehicle.freight_trains},
        {"plane_speed", _settings_game.vehicle.plane_speed}, {"map_width", Map::SizeX()}, {"map_height", Map::SizeY()},
        {"never_expire_airports", _settings_game.station.never_expire_airports}, {"max_bridge_length", _settings_game.construction.max_bridge_length},
        {"never_expire_vehicles", _settings_game.vehicle.never_expire_vehicles}, {"station_noise_level", _settings_game.economy.station_noise_level},
        {"gradual_loading", _settings_game.order.gradual_loading}, {"train_signal_side", _settings_game.construction.train_signal_side},
        {"build_on_slopes", _settings_game.construction.build_on_slopes}, {"wagon_speed_limits", _settings_game.vehicle.wagon_speed_limits},
        {"allow_town_roads", _settings_game.economy.allow_town_roads}, {"generating_world", _generating_world},
        {"improved_load", _settings_game.order.improved_load}, {"dynamic_engines", _settings_game.vehicle.dynamic_engines},
        {"inflation", _settings_game.economy.inflation}};
}
inline void Apply(const Json &value)
{
    if (value.at("map_width").get<uint32_t>() != Map::SizeX() || value.at("map_height").get<uint32_t>() != Map::SizeY()) HostError("fixture map must come from actual input save");
    auto mode = value.at("game_mode").get<uint8_t>();
    auto units = value.at("timekeeping_units").get<uint8_t>();
    auto climate = value.at("climate").get<uint8_t>();
    if (mode > 3 || units > 1 || climate > 3) HostError("invalid fixture enum");
    int32_t year = value.at("starting_year").get<int32_t>();
    int32_t date = value.at("calendar_date").get<int32_t>();
    int32_t economy = value.at("economy_date").get<int32_t>();
    if (year < 0 || year > CalendarTime::MAX_YEAR.base() || date < 0 || date > CalendarTime::MAX_DATE.base() || economy < 0 ||
        economy > (units == TKU_WALLCLOCK ? CalendarTime::MAX_YEAR.base() * 360 + 359 : CalendarTime::MAX_DATE.base())) HostError("invalid fixture date domain");
    _game_mode = static_cast<GameMode>(mode);
    _settings_game.economy.timekeeping_units = static_cast<TimekeepingUnits>(units);
    _settings_game.game_creation.landscape = static_cast<LandscapeType>(climate);
    _settings_game.game_creation.starting_year = TimerGameCalendar::Year{year};
    _settings_game.vehicle.road_side = value.at("road_side").get<uint8_t>();
    _settings_game.vehicle.disable_elrails = value.at("disable_elrails").get<bool>();
    _settings_game.construction.map_height_limit = value.at("height_limit").get<uint8_t>();
    _settings_game.game_creation.snow_line_height = value.at("snowline").get<uint8_t>();
    _settings_game.game_creation.generation_seed = value.at("generation_seed").get<uint32_t>();
    _settings_game.vehicle.freight_trains = value.at("freight_trains").get<uint8_t>();
    _settings_game.vehicle.plane_speed = value.at("plane_speed").get<uint8_t>();
    _settings_game.station.never_expire_airports = value.at("never_expire_airports").get<bool>();
    _settings_game.construction.max_bridge_length = value.at("max_bridge_length").get<uint16_t>();
    _settings_game.vehicle.never_expire_vehicles = value.at("never_expire_vehicles").get<bool>();
    _settings_game.economy.station_noise_level = value.at("station_noise_level").get<bool>();
    _settings_game.order.gradual_loading = value.at("gradual_loading").get<bool>();
    _settings_game.construction.train_signal_side = value.at("train_signal_side").get<uint8_t>();
    _settings_game.construction.build_on_slopes = value.at("build_on_slopes").get<bool>();
    _settings_game.vehicle.wagon_speed_limits = value.at("wagon_speed_limits").get<bool>();
    _settings_game.economy.allow_town_roads = value.at("allow_town_roads").get<bool>();
    _generating_world = value.at("generating_world").get<bool>();
    _settings_game.order.improved_load = value.at("improved_load").get<bool>();
    _settings_game.vehicle.dynamic_engines = value.at("dynamic_engines").get<bool>();
    _settings_game.economy.inflation = value.at("inflation").get<bool>();
    TimerGameCalendar::date = TimerGameCalendar::Date{date};
    TimerGameCalendar::year = TimerGameCalendar::ConvertDateToYMD(TimerGameCalendar::date).year;
    TimerGameCalendar::date_fract = value.at("calendar_fraction").get<uint16_t>();
    TimerGameEconomy::date = TimerGameEconomy::Date{economy};
    TimerGameEconomy::year = TimerGameEconomy::ConvertDateToYMD(TimerGameEconomy::date).year;
    TimerGameEconomy::date_fract = value.at("economy_fraction").get<uint16_t>();
    TimerGameTick::counter = value.at("tick").get<uint64_t>();
    _display_opt = value.at("display").get<uint8_t>();
    _networking = value.at("networking").get<bool>();
}
inline void Prepare(const Json &manifest)
{
    if (!manifest.contains("context")) return;
    if (active || used) HostError("duplicate fixture context");
    const char *destination = std::getenv("OTTD_GRF_CONTEXT_OUTPUT");
    if (destination == nullptr) HostError("missing context output");
    output = destination;
    if (std::ifstream(output).good()) HostError("context output already exists");
    active = true;
    used = true;
    original = Snapshot();
    try {
        Apply(manifest.at("context"));
        size_t index = 2;
        for (const auto &entry : manifest.at("files")) {
            uint8_t palette = entry.at("palette").get<uint8_t>();
            if (palette > 1 || index >= _grfconfig.size()) HostError("invalid selected palette/config");
            _grfconfig[index++]->palette = palette;
        }
    }
    catch (const std::exception &error) { HostError(error.what()); }
    prepared = Snapshot();
}
inline Json Globals()
{
    return {{"rail_costs", {GetRailTypeInfo(RAILTYPE_RAIL)->cost_multiplier, GetRailTypeInfo(RAILTYPE_ELECTRIC)->cost_multiplier,
        GetRailTypeInfo(RAILTYPE_MONO)->cost_multiplier, GetRailTypeInfo(RAILTYPE_MAGLEV)->cost_multiplier}}, {"misc", _misc_grf_features.base()}};
}
inline Json Files()
{
    Json result = Json::array();
    for (const auto &config : _grfconfig) {
        const auto *file = GetFileByFilename(config->filename);
        result.push_back(file ? Json{{"pitch", file->traininfo_vehicle_pitch}, {"width", file->traininfo_vehicle_width}} : Json(nullptr));
    }
    return result;
}
inline void StageStart(GrfLoadingStage stage)
{
    if (!active) return;
    baseline_globals = Globals();
    stages.push_back({{"stage", stage}, {"context", Snapshot()}, {"globals", baseline_globals}, {"files", Files()}});
}
inline void Decision()
{
    if (!active) return;
    bool baseline = (_cur_gps.grfconfig == _grfconfig[0].get() || _cur_gps.grfconfig == _grfconfig[1].get());
    if (baseline && Globals() != baseline_globals) HostError("unexecuted baseline changed control globals");
}
inline void Finish()
{
    if (!active) return;
    Json after = Snapshot();
    for (const char *field : {"calendar_date", "calendar_year", "calendar_fraction", "economy_date", "economy_year", "economy_fraction", "tick", "display", "random", "interactive_random"}) {
        if (prepared.at(field) != after.at(field)) HostError("original loader did not restore fixture clock/RNG");
    }
    Json result = {{"before", original}, {"prepared", prepared}, {"after_native", after}, {"stages", stages}, {"globals", Globals()}, {"files", Files()}};
    Apply(original);
    result["restored"] = Snapshot();
    if (result.at("restored") != original) HostError("fixture context restoration changed original state");
    std::ofstream destination(output);
    destination << result.dump() << '\n';
    if (!destination) HostError("cannot write context observation");
    active = false;
}
}
#endif
