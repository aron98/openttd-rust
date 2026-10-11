// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RAIL_CONSIST_HPP
#define OTTD_REFERENCE_RAIL_CONSIST_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../train.h"
#include "../core/random_func.hpp"
#include "../timer/timer_game_tick.h"
#include <fstream>
namespace ReferenceRailConsist {
inline void Observe()
{
    const char *path = std::getenv("OTTD_RAIL_CONSIST_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(path).good()) throw std::runtime_error("Rail observation already exists");
    nlohmann::json result = {{"phase", "after-load-before-tick"}, {"tick", TimerGameTick::counter},
        {"random", {_random.state[0], _random.state[1]}}, {"rail", nlohmann::json::array()}};
    for (const Train *v : Train::Iterate()) {
        result["rail"].push_back({
            {"id", v->index.base()}, {"engine_id", v->engine_type.base()},
            {"first", v->First()->index.base()},
            {"previous", v->Previous() == nullptr ? VehicleID::Invalid().base() : v->Previous()->index.base()},
            {"next", v->Next() == nullptr ? VehicleID::Invalid().base() : v->Next()->index.base()},
            {"other_multiheaded_part", v->other_multiheaded_part == nullptr ? VehicleID::Invalid().base() : v->other_multiheaded_part->index.base()},
            {"owner", v->owner.base()}, {"cargo_type", v->cargo_type}, {"cargo_subtype", v->cargo_subtype},
            {"sprite_number", v->spritenum}, {"track", v->track}, {"direction", v->direction},
            {"x", v->x_pos}, {"y", v->y_pos}, {"z", v->z_pos},
            {"tile", v->tile.base()}, {"subtype", v->subtype}, {"flags", v->flags.base()},
            {"capacity", v->cargo_cap}, {"stored_count", v->cargo.StoredCount()},
            {"current_speed", v->cur_speed}, {"acceleration", v->acceleration},
            {"vehstatus", v->vehstatus.base()}, {"railtypes", v->railtypes.base()},
            {"compatible_railtypes", v->compatible_railtypes.base()},
            {"first_engine", v->gcache.first_engine.base()},
            {"vehicle_length", v->gcache.cached_veh_length}, {"total_length", v->gcache.cached_total_length},
            {"max_speed", v->vcache.cached_max_speed}, {"cargo_age_period", v->vcache.cached_cargo_age_period},
            {"visual_effect", v->vcache.cached_vis_effect}, {"weight", v->gcache.cached_weight},
            {"slope_resistance", v->gcache.cached_slope_resistance}, {"axle_resistance", v->gcache.cached_axle_resistance},
            {"power", v->gcache.cached_power}, {"max_tractive_effort", v->gcache.cached_max_te},
            {"max_track_speed", v->gcache.cached_max_track_speed}, {"air_drag", v->gcache.cached_air_drag},
            {"last_speed", v->gcache.last_speed}, {"trip_occupancy", v->trip_occupancy},
            {"user_def_data", v->tcache.user_def_data}, {"tilt", v->tcache.cached_tilt},
            {"curve_speed_mod", v->tcache.cached_curve_speed_mod}, {"max_curve_speed", v->tcache.cached_max_curve_speed},
            {"override_present", v->tcache.cached_override != nullptr}, {"colourmap", v->colourmap},
        });
        if (v->IsFrontEngine()) result["rail"].back()["braking_acceleration"] = v->GetAcceleration();
    }
    std::ofstream output(path);
    output << result.dump(2) << '\n';
    if (!output) throw std::runtime_error("Rail observation write failed");
}
}
#endif
