// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_RUNTIME_ROAD_FIXTURE_HPP
#define OTTD_REFERENCE_RUNTIME_ROAD_FIXTURE_HPP
#include "../roadveh.h"
#include "../road_cmd.h"
#include "../vehicle_cmd.h"
#include "../command_func.h"
#include "../company_base.h"
#include "../engine_base.h"
#include "../economy_func.h"
#include "../core/backup_type.hpp"
#include "../newgrf_config.h"
#include <fstream>
namespace ReferenceRuntimeRoadFixture {
inline void Prepare()
{
    const char *manifest = std::getenv("OTTD_RUNTIME_ROAD_PREPARE");
    if (manifest == nullptr || _game_mode == GM_MENU) return;
    static bool prepared = false;
    if (prepared) return;
    if (std::ifstream(manifest).good()) throw std::runtime_error("Road runtime fixture manifest already exists");
    if (Vehicle::GetNumItems() != 0 || !_grfconfig.empty() || !Company::IsValidID(CompanyID(0))) throw std::runtime_error("Road fixture requires vanilla vehicle-free company 0");
    AutoRestoreBackup company(_current_company, CompanyID(0));
    TileIndex depot = INVALID_TILE;
    for (uint32_t id = 1; id < Map::Size(); ++id) {
        const TileIndex tile(id);
        if (Command<CMD_BUILD_ROAD_DEPOT>::Do({}, tile, ROADTYPE_ROAD, DIAGDIR_NE).Failed()) continue;
        const auto cost = Command<CMD_BUILD_ROAD_DEPOT>::Do(DoCommandFlag::Execute, tile, ROADTYPE_ROAD, DIAGDIR_NE);
        if (cost.Failed()) throw std::runtime_error("Road depot execution failed after successful test");
        SubtractMoneyFromCompany(cost);
        depot = tile;
        break;
    }
    if (depot == INVALID_TILE) throw std::runtime_error("Road fixture has no buildable depot tile");
    uint32_t built = 0;
    for (Engine *e : Engine::IterateType(VEH_ROAD)) {
        if (!e->info.climates.Test(_settings_game.game_creation.landscape)) continue;
        e->company_avail.Set(CompanyID(0));
        auto [cost, id, capacity, mail_capacity, capacities] = Command<CMD_BUILD_VEHICLE>::Do(DoCommandFlag::Execute, depot, e->index, false, INVALID_CARGO, INVALID_CLIENT_ID);
        if (cost.Failed() || !Vehicle::IsValidID(id)) throw std::runtime_error("Native road fixture vehicle construction failed");
        SubtractMoneyFromCompany(cost);
        RoadVehicle *v = RoadVehicle::Get(id);
        v->max_age = TimerGameCalendar::Date{12345};
        v->reliability = 45678;
        v->cur_speed = built % 100;
        if (built % 7 == 0) v->cargo_cap = 0;
        uint16_t amount = 0;
        switch (built % 6) {
            case 0: amount = 0; break;
            case 1: amount = 1; break;
            case 2: amount = v->cargo_cap / 2; break;
            case 3: amount = v->cargo_cap; break;
            case 4: amount = v->cargo_cap + 17; break;
            case 5: amount = 60000; break;
        }
        if (amount != 0) {
            if (!CargoPacket::CanAllocateItem()) throw std::runtime_error("Cargo pool full");
            v->cargo.Append(new CargoPacket(amount, 0, StationID::Invalid(), depot, Money{0}));
        }
        if (!CargoPacket::CanAllocateItem()) throw std::runtime_error("Cargo pool full");
        v->cargo.Append(new CargoPacket(3, 0, StationID::Invalid(), depot, Money{0}), VehicleCargoList::MTA_LOAD);
        ++built;
    }
    if (built < 6) throw std::runtime_error("Insufficient native road fixture engines");
    _pause_mode = PauseMode::Normal;
    std::ofstream output(manifest);
    output << "phase=native-fixture-preparation\nvehicles=" << built << "\ndepot=" << depot.base()
        << "\ncommands=CMD_BUILD_ROAD_DEPOT,CMD_BUILD_VEHICLE\nfixture_fields=company_avail,max_age,reliability,cur_speed,cargo_cap,cargo_packets\n";
    if (!output) throw std::runtime_error("Road fixture manifest write failed");
    prepared = true;
}
}
#endif
