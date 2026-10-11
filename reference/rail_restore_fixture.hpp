// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RAIL_RESTORE_FIXTURE_HPP
#define OTTD_REFERENCE_RAIL_RESTORE_FIXTURE_HPP
#include "../rail_cmd.h"
#include "../vehicle_cmd.h"
#include "../command_func.h"
#include "../company_base.h"
#include "../engine_base.h"
#include "../core/backup_type.hpp"
#include "../newgrf_config.h"
#include "../train.h"
#include <fstream>
namespace ReferenceRailRestoreFixture {
inline VehicleID fixture_vehicle = VehicleID::Invalid();
inline void Prepare()
{
    const char *manifest = std::getenv("OTTD_RAIL_RESTORE_PREPARE");
    if (manifest == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(manifest).good()) throw std::runtime_error("Rail fixture already exists");
    if (Vehicle::GetNumItems() != 0 || !_grfconfig.empty() || !Company::IsValidID(CompanyID(0))) {
        throw std::runtime_error("Rail fixture requires vanilla vehicle-free company 0");
    }
    AutoRestoreBackup company(_current_company, CompanyID(0));
    Company::Get(CompanyID(0))->money = Money{100000000};
    const TileIndex depot = TileXY(8, 8);
    if (Command<CMD_BUILD_TRAIN_DEPOT>::Do(DoCommandFlag::Execute, depot, RAILTYPE_RAIL, DIAGDIR_NE).Failed()) {
        throw std::runtime_error("Rail depot construction failed");
    }
    const char *engine_arg = std::getenv("OTTD_RAIL_RESTORE_ENGINE");
    if (engine_arg == nullptr) throw std::runtime_error("Rail fixture engine missing");
    const EngineID engine_id(std::stoi(engine_arg));
    if (!Engine::IsValidID(engine_id)) throw std::runtime_error("Invalid rail fixture engine");
    Engine *engine = Engine::Get(engine_id);
    if (engine->type != VEH_TRAIN || engine->VehInfo<RailVehicleInfo>().railveh_type != RAILVEH_SINGLEHEAD) {
        throw std::runtime_error("Rail fixture requires single-headed engine");
    }
    engine->company_avail.Set(CompanyID(0));
    auto [cost, id, capacity, mail_capacity, capacities] = Command<CMD_BUILD_VEHICLE>::Do(
        DoCommandFlag::Execute, depot, engine_id, false, INVALID_CARGO, INVALID_CLIENT_ID);
    if (cost.Failed() || !Vehicle::IsValidID(id)) throw std::runtime_error("Rail engine construction failed");
    Train *v = Train::Get(id);
    if (v->Next() != nullptr || !v->IsFrontEngine()) throw std::runtime_error("Unexpected native consist");
    for (uint x = 2; x < 8; ++x) {
        if (Command<CMD_BUILD_SINGLE_RAIL>::Do(DoCommandFlag::Execute, TileXY(x, 8), RAILTYPE_RAIL, TRACK_X, false).Failed()) {
            throw std::runtime_error("Rail track construction failed");
        }
    }
    if (Command<CMD_START_STOP_VEHICLE>::Do(DoCommandFlag::Execute, id, false).Failed()) {
        throw std::runtime_error("Rail start command failed");
    }
    fixture_vehicle = id;
    _pause_mode = {};
    std::ofstream output(manifest);
    output << "phase=native-fixture-preparation\nengine=" << engine_id.base() << "\nvehicle=" << id.base()
        << "\ncommands=CMD_BUILD_TRAIN_DEPOT,CMD_BUILD_VEHICLE,CMD_BUILD_SINGLE_RAIL,CMD_START_STOP_VEHICLE"
        << "\nfixture_fields=company_money,engine_company_avail,pause_mode\n";
    if (!output) throw std::runtime_error("Rail fixture manifest write failed");
}
inline void Advance()
{
    if (fixture_vehicle == VehicleID::Invalid()) return;
    Train *v = Train::Get(fixture_vehicle);
    if (!IsPlainRailTile(v->tile)) return;
    AutoRestoreBackup company(_current_company, v->owner);
    if (!v->vehstatus.Test(VehState::Stopped)) {
        if (Command<CMD_START_STOP_VEHICLE>::Do(DoCommandFlag::Execute, v->index, false).Failed()) {
            throw std::runtime_error("Rail stop command failed");
        }
    }
    if (v->cur_speed != 0) return;
    _pause_mode = PauseMode::Normal;
    std::ofstream output(std::string(std::getenv("OTTD_RAIL_RESTORE_PREPARE")) + ".stopped");
    output << "native_movement_stopped_on_plain_rail=" << v->tile.base() << '\n';
    if (!output) throw std::runtime_error("Rail stopped manifest write failed");
    fixture_vehicle = VehicleID::Invalid();
}
}
#endif
