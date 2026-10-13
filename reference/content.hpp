// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CONTENT_HPP
#define OTTD_REFERENCE_CONTENT_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../engine_base.h"
#include "../cargotype.h"
#include "../economy_func.h"
#include "../economy_type.h"
#include "../settings_type.h"
#include "../newgrf_config.h"
#include <fstream>
namespace ReferenceContent {
using Json = nlohmann::json;
inline Json PriceName(Price price)
{
    static constexpr const char *names[] = {"StationValue", "BuildRail", "BuildRoad", "BuildSignals", "BuildBridge", "BuildDepotTrain", "BuildDepotRoad", "BuildDepotShip", "BuildTunnel", "BuildStationRail", "BuildStationRailLength", "BuildStationAirport", "BuildStationBus", "BuildStationTruck", "BuildStationDock", "BuildVehicleTrain", "BuildVehicleWagon", "BuildVehicleAircraft", "BuildVehicleRoad", "BuildVehicleShip", "BuildTrees", "Terraform", "ClearGrass", "ClearRough", "ClearRocks", "ClearFields", "ClearTrees", "ClearRail", "ClearSignals", "ClearBridge", "ClearDepotTrain", "ClearDepotRoad", "ClearDepotShip", "ClearTunnel", "ClearWater", "ClearStationRail", "ClearStationAirport", "ClearStationBus", "ClearStationTruck", "ClearStationDock", "ClearHouse", "ClearRoad", "RunningTrainSteam", "RunningTrainDiesel", "RunningTrainElectric", "RunningAircraft", "RunningRoadveh", "RunningShip", "BuildIndustry", "ClearIndustry", "BuildObject", "ClearObject", "BuildWaypointRail", "ClearWaypointRail", "BuildWaypointBuoy", "ClearWaypointBuoy", "TownAction", "BuildFoundation", "BuildIndustryRaw", "BuildTown", "BuildCanal", "ClearCanal", "BuildAqueduct", "ClearAqueduct", "BuildLock", "ClearLock", "InfrastructureRail", "InfrastructureRoad", "InfrastructureWater", "InfrastructureStation", "InfrastructureAirport"};
    return price == INVALID_PRICE ? Json(nullptr) : Json(names[price]);
}
inline Json Label(const std::variant<CargoLabel, MixedCargoType> &label)
{
    if (const auto *fixed = std::get_if<CargoLabel>(&label)) return Json{{"Fixed", fixed->base()}};
    switch (std::get<MixedCargoType>(label)) {
        case MCT_LIVESTOCK_FRUIT: return "LivestockFruit";
        case MCT_GRAIN_WHEAT_MAIZE: return "GrainWheatMaize";
        case MCT_VALUABLES_GOLD_DIAMONDS: return "ValuablesGoldDiamonds";
        default: throw std::runtime_error("Unknown native mixed cargo label");
    }
}
inline Json Info(const EngineInfo &i)
{
    return {
        {"base_intro", i.base_intro.base()}, {"lifelength", i.lifelength.base()}, {"base_life", i.base_life.base()},
        {"decay_speed", i.decay_speed}, {"load_amount", i.load_amount}, {"climates", i.climates.base()},
        {"cargo_type", i.cargo_type}, {"cargo_label", Label(i.cargo_label)}, {"refit_mask", i.refit_mask},
        {"refit_cost", i.refit_cost}, {"misc_flags", i.misc_flags.base()}, {"callback_mask", i.callback_mask.base()},
        {"retire_early", i.retire_early}, {"extra_flags", i.extra_flags.base()}, {"cargo_age_period", i.cargo_age_period}, {"variant_id", i.variant_id.base()}
    };
}
inline Json Vehicle(const RailVehicleInfo &v)
{
    return {{"Rail", {
        {"image_index", v.image_index},
        {"railveh_type", v.railveh_type},
        {"cost_factor", v.cost_factor},
        {"railtypes", v.railtypes.base()},
        {"intended_railtypes", v.intended_railtypes.base()},
        {"ai_passenger_only", v.ai_passenger_only},
        {"max_speed", v.max_speed},
        {"power", v.power},
        {"weight", v.weight},
        {"running_cost", v.running_cost},
        {"running_cost_class", PriceName(v.running_cost_class)},
        {"engclass", v.engclass},
        {"capacity", v.capacity},
        {"pow_wag_power", v.pow_wag_power},
        {"pow_wag_weight", v.pow_wag_weight},
        {"visual_effect", v.visual_effect},
        {"shorten_factor", v.shorten_factor},
        {"tractive_effort", v.tractive_effort},
        {"air_drag", v.air_drag},
        {"user_def_data", v.user_def_data},
        {"curve_speed_mod", v.curve_speed_mod},
    }}};
}
inline Json Vehicle(const RoadVehicleInfo &v)
{
    return {{"Road", {
        {"image_index", v.image_index},
        {"cost_factor", v.cost_factor},
        {"running_cost", v.running_cost},
        {"running_cost_class", PriceName(v.running_cost_class)},
        {"sfx", v.sfx},
        {"max_speed", v.max_speed},
        {"capacity", v.capacity},
        {"weight", v.weight},
        {"power", v.power},
        {"tractive_effort", v.tractive_effort},
        {"air_drag", v.air_drag},
        {"visual_effect", v.visual_effect},
        {"shorten_factor", v.shorten_factor},
        {"roadtype", v.roadtype},
    }}};
}
inline Json Vehicle(const ShipVehicleInfo &v)
{
    return {{"Ship", {
        {"image_index", v.image_index},
        {"cost_factor", v.cost_factor},
        {"running_cost", v.running_cost},
        {"acceleration", v.acceleration},
        {"max_speed", v.max_speed},
        {"capacity", v.capacity},
        {"sfx", v.sfx},
        {"old_refittable", v.old_refittable},
        {"visual_effect", v.visual_effect},
        {"ocean_speed_frac", v.ocean_speed_frac},
        {"canal_speed_frac", v.canal_speed_frac},
    }}};
}
inline Json Vehicle(const AircraftVehicleInfo &v)
{
    return {{"Aircraft", {
        {"image_index", v.image_index},
        {"cost_factor", v.cost_factor},
        {"running_cost", v.running_cost},
        {"subtype", v.subtype},
        {"sfx", v.sfx},
        {"max_speed", v.max_speed},
        {"acceleration", v.acceleration},
        {"mail_capacity", v.mail_capacity},
        {"passenger_capacity", v.passenger_capacity},
        {"max_range", v.max_range},
    }}};
}
inline void Observe()
{
    const char *path = std::getenv("OTTD_CONTENT_PATH");
    if (path == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(path).good()) throw std::runtime_error("Content observation already exists");
    if (!_grfconfig.empty()) throw std::runtime_error("Vanilla content observer received NewGRF world");
    static constexpr const char *climates[] = {"Temperate", "Arctic", "Tropic", "Toyland"};
    Json result = {{"climate", climates[to_underlying(_settings_game.game_creation.landscape)]},
        {"engines", Json::array()}, {"cargo", Json::array()}, {"prices", {{"values", Json::array()}, {"max_loan", static_cast<int64_t>(_economy.max_loan)}}}};
    for (const Engine *e : Engine::Iterate()) {
        Json vehicle;
        switch (e->type) {
            case VEH_TRAIN: vehicle = Vehicle(e->VehInfo<RailVehicleInfo>()); break;
            case VEH_ROAD: vehicle = Vehicle(e->VehInfo<RoadVehicleInfo>()); break;
            case VEH_SHIP: vehicle = Vehicle(e->VehInfo<ShipVehicleInfo>()); break;
            case VEH_AIRCRAFT: vehicle = Vehicle(e->VehInfo<AircraftVehicleInfo>()); break;
            default: throw std::runtime_error("Unexpected native engine type");
        }
        result["engines"].push_back({{"id", e->index.base()}, {"local_id", e->grf_prop.local_id}, {"info", Info(e->info)}, {"vehicle", std::move(vehicle)}});
    }
    for (CargoType id = 0; id < NUM_CARGO; ++id) {
        const CargoSpec &c = *CargoSpec::Get(id);
        result["cargo"].push_back({
            {"label", c.label.base()},
            {"bitnum", c.bitnum},
            {"legend_colour", c.legend_colour.p},
            {"rating_colour", c.rating_colour.p},
            {"weight", c.weight},
            {"multiplier", c.multiplier},
            {"classes", c.classes.base()},
            {"initial_payment", c.initial_payment},
            {"transit_periods", c.transit_periods},
            {"is_freight", c.is_freight},
            {"town_acceptance_effect", c.town_acceptance_effect},
            {"town_production_effect", c.town_production_effect},
            {"town_production_multiplier", c.town_production_multiplier},
            {"callback_mask", c.callback_mask.base()},
            {"current_payment", static_cast<int64_t>(c.current_payment)},
        });
    }
    for (Price price = PR_BEGIN; price < PR_END; price++) result["prices"]["values"].push_back(static_cast<int64_t>(_price[price]));
    std::ofstream output(path);
    output << result.dump(2) << '\n';
    if (!output) throw std::runtime_error("Content observation write failed");
}
}
#endif
