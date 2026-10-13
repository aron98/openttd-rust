// SPDX-License-Identifier: GPL-2.0-only
// SOURCE-ONLY DRAFT: included beside reference_runtime_road.hpp by root's replay observer.
#ifndef OTTD_REFERENCE_MOVEMENT_SNAPSHOT_HPP
#define OTTD_REFERENCE_MOVEMENT_SNAPSHOT_HPP
#include "reference_runtime_road.hpp"
#include "reference_cargo_runtime.hpp"
#include "reference_job_annotations.hpp"
#include "reference_house_cache.hpp"
#include "reference_industry_cache_snapshot.hpp"
#include "reference_road_station_runtime.hpp"
#include "../economy_base.h"
#include "../linkgraph/linkgraphjob.h"
namespace ReferenceMovement {
using Json = nlohmann::json;
inline Json Transport()
{
    Json payments = Json::object(), jobs = Json::object();
    for (const CargoPayment *payment : CargoPayment::Iterate()) {
        payments[std::to_string(payment->index.base())] = payment->current_station.base();
    }
    for (LinkGraphJob *job : LinkGraphJob::Iterate()) {
        const std::string id = std::to_string(job->index.base());
        jobs[id] = nullptr;
        // Acquire completion before reading worker-owned annotations. Never wait,
        // join, mutate scheduling or observe the incomplete vectors.
        if (!job->IsJobCompleted()) continue;
        jobs[id] = ReferenceJobs::Completed(job);
    }
    return {{"payments", ReferenceOrderState::PoolState(_cargo_payment_pool)},
        {"graphs", ReferenceOrderState::PoolState(_link_graph_pool)},
        {"jobs", ReferenceOrderState::PoolState(_link_graph_job_pool)},
        {"payment_stations", payments}, {"job_results", jobs}, {"cargo", ReferenceCargoRuntime::Snapshot()}};
}
inline Json Snapshot()
{
    Json vehicles = Json::array();
    bool supported_runtime = true;
    for (const Vehicle *v : Vehicle::Iterate()) {
        Json row = {{"id", v->index.base()}, {"type", v->type},
            {"tile", v->tile.base()}, {"xyz", {v->x_pos, v->y_pos, v->z_pos}},
            {"direction", v->direction}, {"speed", v->cur_speed}, {"subspeed", v->subspeed},
            {"progress", v->progress}, {"status", v->vehstatus.base()},
            {"tick_counter", v->tick_counter}, {"day_counter", v->day_counter},
            {"running_ticks", v->running_ticks}, {"cargo_age_counter", v->cargo_age_counter},
            {"motion_counter", v->motion_counter}, {"load_unload_ticks", v->load_unload_ticks},
            {"dest_tile", v->dest_tile.base()}, {"current_order_time", v->current_order_time},
            {"order_type", v->current_order.GetType()},
            {"real_order_index", v->cur_real_order_index},
            {"implicit_order_index", v->cur_implicit_order_index},
            {"stored_count", v->cargo.StoredCount()}, {"random_bits", v->random_bits},
            {"hash_tile_next", v->hash_tile_next == nullptr ? Json(nullptr) : Json(v->hash_tile_next->index.base())},
            {"hash_tile_attached", v->hash_tile_current != nullptr}};
        if (v->type == VEH_ROAD) {
            const RoadVehicle *r = RoadVehicle::From(v);
            supported_runtime = supported_runtime && r->IsFrontEngine() && r->Next() == nullptr;
            Json path = Json::array();
            for (const auto &p : r->path) path.push_back({{"tile", p.tile.base()}, {"trackdir", p.trackdir}});
            row["road"] = {{"state", r->state}, {"frame", r->frame},
                {"blocked_ctr", r->blocked_ctr}, {"reverse_ctr", r->reverse_ctr},
                {"overtaking", r->overtaking}, {"overtaking_ctr", r->overtaking_ctr},
                {"crashed_ctr", r->crashed_ctr}, {"saved_path", path}};
        } else {
            supported_runtime = false;
        }
        vehicles.push_back(std::move(row));
    }
    Json companies = Json::array();
    for (const Company *c : Company::Iterate()) companies.push_back({{"id", c->index.base()},
        {"money", static_cast<int64_t>(c->money)}, {"money_fraction", c->money_fraction},
        {"loan", static_cast<int64_t>(c->current_loan)}});
    return {{"vehicles", vehicles}, {"companies", companies},
        {"transport", Transport()}, {"house_caches", ReferenceHouseCache::Snapshot()}, {"industry_caches", ReferenceIndustryCache::Snapshot()}, {"stations", supported_runtime ? ReferenceRoadStationRuntime::Snapshot() : Json(nullptr)},
        {"runtime_road", supported_runtime ? ReferenceRuntimeRoad::Live() : Json(nullptr)},
        {"runtime_road_supported", supported_runtime}};
}
}
#endif
