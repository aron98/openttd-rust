// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RUNTIME_ROAD_HPP
#define OTTD_REFERENCE_RUNTIME_ROAD_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../roadveh.h"
#include "../engine_base.h"
#include <fstream>
namespace ReferenceRuntimeRoad {
using Json = nlohmann::json;
inline void Observe()
{
    const char *path = std::getenv("OTTD_RUNTIME_ROAD_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(path).good()) throw std::runtime_error("Road runtime observation already exists");
    Json result = {{"road", Json::array()}, {"saved", Json::array()}};
    for (const Vehicle *vehicle : Vehicle::Iterate()) {
        if (vehicle->type != VEH_ROAD) throw std::runtime_error("Non-road vehicle in road runtime scenario");
        const RoadVehicle *v = RoadVehicle::From(vehicle);
        if (!v->IsFrontEngine() || v->Next() != nullptr) throw std::runtime_error("Unsupported articulated road vehicle");
        result["road"].push_back({
            {"id", v->index.base()},
            {"road_type", v->roadtype},
            {"compatible_roadtypes", v->compatible_roadtypes.base()},
            {"first_engine", v->gcache.first_engine == EngineID::Invalid() ? Json(nullptr) : Json(v->gcache.first_engine.base())},
            {"vehicle_length", v->gcache.cached_veh_length},
            {"total_length", v->gcache.cached_total_length},
            {"max_speed", v->vcache.cached_max_speed},
            {"cargo_age_period", v->vcache.cached_cargo_age_period},
            {"visual_effect", v->vcache.cached_vis_effect},
            {"weight", v->gcache.cached_weight},
            {"slope_resistance", v->gcache.cached_slope_resistance},
            {"axle_resistance", v->gcache.cached_axle_resistance},
            {"power", v->gcache.cached_power},
            {"max_tractive_effort", v->gcache.cached_max_te},
            {"max_track_speed", v->gcache.cached_max_track_speed},
            {"air_drag", v->gcache.cached_air_drag},
            {"last_speed", v->gcache.last_speed},
            {"trip_occupancy", v->trip_occupancy},
        });
        const Engine *e = v->GetEngine();
        result["saved"].push_back({{"id", v->index.base()}, {"engine_id", v->engine_type.base()},
            {"tile", v->tile.base()}, {"cargo_type", v->cargo_type}, {"capacity", v->cargo_cap},
            {"stored_count", v->cargo.StoredCount()}, {"current_speed", v->cur_speed},
            {"reliability", v->reliability}, {"max_age", v->max_age.base()},
            {"engine_reliability", e->reliability}, {"engine_decay", e->reliability_spd_dec},
            {"engine_age", e->age}, {"engine_company_availability", e->company_avail.base()}});
    }
    std::ofstream output(path);
    output << result.dump(2) << '\n';
    if (!output) throw std::runtime_error("Road runtime observation write failed");
}
}
#endif
