// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_PREPARE_HPP
#define OTTD_REFERENCE_MOVEMENT_PREPARE_HPP
#include "../engine_func.h"
#include "../company_func.h"
namespace ReferenceMovement {
inline Json PreparationState()
{
    Json companies = Json::array();
    for (const Company *c : Company::Iterate()) companies.push_back({{"id", c->index.base()},
        {"money", static_cast<int64_t>(c->money)}, {"fraction", c->money_fraction},
        {"available", static_cast<int64_t>(GetAvailableMoney(c->index))}, {"loan", static_cast<int64_t>(c->current_loan)}});
    return {{"companies", companies}, {"allocation", ReferenceRuntimeRoad::Live()}};
}
inline Json PurchaseFeasibility(const Json &request, uint64_t reservations)
{
    const auto id = EngineID(static_cast<uint16_t>(Bounded(request.at("command").at("engine"), UINT16_MAX)));
    const auto company = CompanyID(static_cast<uint8_t>(Bounded(request.at("company"), 254)));
    const Engine *engine = Engine::GetIfValid(id);
    ReferenceWorld::Require(engine != nullptr && engine->type == VEH_ROAD && Company::IsValidID(company), "movement purchase engine/company invalid");
    const bool buildable = IsEngineBuildable(id, VEH_ROAD, company);
    const int64_t cost = static_cast<int64_t>(engine->GetCost());
    const int64_t available = static_cast<int64_t>(GetAvailableMoney(company));
    // Divide available funds to avoid overflowing a host gross-cost multiplication.
    const bool affordable = cost > 0 && available >= 0 && reservations + 1 <= static_cast<uint64_t>(available / cost);
    return {{"engine", id.base()}, {"company", company.base()}, {"buildable", buildable}, {"unit_cost", cost},
        {"available", available}, {"purchase_count", reservations + 1}, {"affordable_before_sales", affordable},
        {"state", PreparationState()}};
}
inline uint32_t PurchasedID(const Json &receipt)
{
    return static_cast<uint32_t>(Bounded(receipt.at("receipt").at("returns").at("exec").at("vehicle"), UINT32_MAX - 1));
}
inline void ReservedVehicle(uint32_t id)
{
    const Vehicle *v = Vehicle::GetIfValid(VehicleID(id));
    ReferenceWorld::Require(v != nullptr && v->type == VEH_ROAD && v->owner == CompanyID(0), "reservation identity/owner");
    const RoadVehicle *r = RoadVehicle::From(v);
    ReferenceWorld::Require(r->IsFrontEngine() && r->Next() == nullptr && r->IsStoppedInDepot()
        && r->tile == TileIndex(673), "reservation must remain singlepart stopped in original depot");
}
}
#endif
