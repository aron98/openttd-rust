// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RAIL_CONSIST_FIXTURE_HPP
#define OTTD_REFERENCE_RAIL_CONSIST_FIXTURE_HPP
#include "../rail_cmd.h"
#include "../train_cmd.h"
#include "../vehicle_cmd.h"
#include "../command_func.h"
#include "../company_base.h"
#include "../engine_base.h"
#include "../core/backup_type.hpp"
#include "../newgrf_config.h"
#include "../train.h"
#include "../timer/timer_game_tick.h"
#include <fstream>
#include <sstream>
namespace ReferenceRailConsistFixture {
inline VehicleID fixture_vehicle = VehicleID::Invalid();
inline void Prepare()
{
    const char *manifest = std::getenv("OTTD_RAIL_CONSIST_PREPARE");
    if (manifest == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(manifest).good()) throw std::runtime_error("Consist fixture already exists");
    if (Vehicle::GetNumItems() != 0 || !_grfconfig.empty() || !Company::IsValidID(CompanyID(0))) {
        throw std::runtime_error("Consist fixture requires vanilla vehicle-free company 0");
    }
    const char *roster = std::getenv("OTTD_RAIL_CONSIST_ENGINE");
    if (roster == nullptr) throw std::runtime_error("Consist roster missing");
    AutoRestoreBackup company(_current_company, CompanyID(0));
    Company::Get(CompanyID(0))->money = Money{100000000};
    const TileIndex depot = TileXY(8, 8);
    if (Command<CMD_BUILD_TRAIN_DEPOT>::Do(DoCommandFlag::Execute, depot, RAILTYPE_RAIL, DIAGDIR_NE).Failed()) {
        throw std::runtime_error("Consist depot construction failed");
    }
    std::ofstream output(manifest);
    output << "phase=native-consist-preparation\nroster=" << roster
        << "\nfixture_fields=company_money,engine_company_avail,pause_mode\n";
    for (const Engine *e : Engine::IterateType(VEH_TRAIN)) {
        if (e->info.callback_mask.Any()) throw std::runtime_error("Expected callback-free vanilla engines");
    }
    std::istringstream inputs(roster);
    std::string item;
    while (std::getline(inputs, item, ',')) {
        const EngineID engine_id(std::stoi(item));
        if (!Engine::IsValidID(engine_id)) throw std::runtime_error("Invalid consist engine");
        Engine *engine = Engine::Get(engine_id);
        if (engine->type != VEH_TRAIN || !engine->VehInfo<RailVehicleInfo>().railtypes.Test(RAILTYPE_RAIL)) {
            throw std::runtime_error("Consist requires original normal rail units");
        }
        engine->company_avail.Set(CompanyID(0));
        auto [cost, id, capacity, mail_capacity, capacities] = Command<CMD_BUILD_VEHICLE>::Do(
            DoCommandFlag::Execute, depot, engine_id, false, INVALID_CARGO, INVALID_CLIENT_ID);
        if (cost.Failed() || !Vehicle::IsValidID(id)) throw std::runtime_error("Consist unit construction failed");
        output << "CMD_BUILD_VEHICLE engine=" << engine_id.base() << " id=" << id.base() << '\n';
        if (fixture_vehicle == VehicleID::Invalid()) {
            if (!Train::Get(id)->IsFrontEngine()) throw std::runtime_error("First unit must be a locomotive");
            fixture_vehicle = id;
        } else {
            const VehicleID destination = Train::Get(fixture_vehicle)->Last()->index;
            if (Command<CMD_MOVE_RAIL_VEHICLE>::Do(DoCommandFlag::Execute, id, destination, true).Failed()) {
                throw std::runtime_error("Original consist attachment failed");
            }
            output << "CMD_MOVE_RAIL_VEHICLE src=" << id.base() << " dest=" << destination.base() << " chain=1\n";
        }
    }
    for (uint x = 2; x < 8; ++x) {
        if (Command<CMD_BUILD_SINGLE_RAIL>::Do(DoCommandFlag::Execute, TileXY(x, 8), RAILTYPE_RAIL, TRACK_X, false).Failed()) {
            throw std::runtime_error("Consist track construction failed");
        }
    }
    for (const Train *v = Train::Get(fixture_vehicle); v != nullptr; v = v->Next()) {
        if (v->IsArticulatedPart()) throw std::runtime_error("Unexpected vanilla articulated part");
        output << "actual_part id=" << v->index.base() << " engine=" << v->engine_type.base()
            << " subtype=" << (uint)v->subtype << " cargo=" << (uint)v->cargo_type << '\n';
    }
    if (Command<CMD_START_STOP_VEHICLE>::Do(DoCommandFlag::Execute, fixture_vehicle, false).Failed()) {
        throw std::runtime_error("Consist start command failed");
    }
    _pause_mode = {};
    output << "CMD_START_STOP_VEHICLE start tick=" << TimerGameTick::counter << '\n';
    if (!output) throw std::runtime_error("Consist manifest write failed");
}
inline void Advance()
{
    if (fixture_vehicle == VehicleID::Invalid()) return;
    Train *v = Train::Get(fixture_vehicle);
    for (const Train *u = v; u != nullptr; u = u->Next()) {
        if (!IsPlainRailTile(u->tile)) return;
    }
    AutoRestoreBackup company(_current_company, v->owner);
    if (!v->vehstatus.Test(VehState::Stopped)) {
        if (Command<CMD_START_STOP_VEHICLE>::Do(DoCommandFlag::Execute, v->index, false).Failed()) {
            throw std::runtime_error("Consist stop command failed");
        }
    }
    if (v->cur_speed != 0) return;
    /* Let original effect ticks expire smoke created during authentic movement. */
    for (const Vehicle *effect : Vehicle::Iterate()) {
        if (effect->type == VEH_EFFECT) return;
        if (effect->type != VEH_TRAIN) throw std::runtime_error("Unexpected non-train fixture vehicle");
    }
    _pause_mode = PauseMode::Normal;
    std::ofstream output(std::string(std::getenv("OTTD_RAIL_CONSIST_PREPARE")) + ".stopped");
    output << "native_consist_stopped_tick=" << TimerGameTick::counter << '\n';
    for (const Train *u = v; u != nullptr; u = u->Next()) {
        output << "part=" << u->index.base() << " tile=" << u->tile.base() << " track=" << (uint)u->track << '\n';
    }
    if (!output) throw std::runtime_error("Consist stopped manifest write failed");
    fixture_vehicle = VehicleID::Invalid();
}
}
#endif
