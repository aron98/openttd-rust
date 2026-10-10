// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_SINK_HPP
#define OTTD_REFERENCE_MOVEMENT_SINK_HPP
#include "../reference_movement_hooks.hpp"
#include "reference_movement_snapshot.hpp"
#include "../map_func.h"
#include <chrono>
#include <string_view>
namespace ReferenceMovement {
Json Spatial();
enum class Failure { None, Output, Snapshot, EventBudget, StorageBudget };
inline Failure failure = Failure::None;
inline std::ofstream events;
inline uint64_t sequence = 0;
inline uint64_t action = 0;
inline uint64_t call = 0;
inline uint64_t event_bytes = 0;
inline uint64_t max_events = 0;
inline uint64_t max_event_bytes = 0;
inline bool tracing = true;
inline const char *FailureName()
{
    switch (failure) {
        case Failure::None: return "none";
        case Failure::Output: return "observer_output_failure";
        case Failure::Snapshot: return "observer_snapshot_failure";
        case Failure::EventBudget: return "host_event_budget";
        case Failure::StorageBudget: return "host_storage_budget";
    }
    return "observer_invalid_failure";
}
inline Json TileBytes(uint32_t id)
{
    if (id >= Map::Size()) return nullptr;
    Tile tile(id);
    return {tile.type(), tile.height(), tile.m1(), tile.m2(), tile.m3(),
        tile.m4(), tile.m5(), tile.m6(), tile.m7(), tile.m8()};
}
inline Json Live()
{
    Json value = ReferenceReplay::Runtime();
    value["local_company"] = _local_company.base();
    value["calendar_sub_date_fract"] = TimerGameCalendar::sub_date_fract;
    value["economy_days_since_last_month"] = TimerGameEconomy::days_since_last_month;
    value["tileloop_cursor"] = _cur_tileloop_tile.base();
    value["movement"] = Snapshot();
    value["road_side"] = _settings_game.vehicle.road_side;
    value["acceleration_model"] = _settings_game.vehicle.roadveh_acceleration_model;
    value["smoke_amount"] = _settings_game.vehicle.smoke_amount;
    value["vehicle_sound_enabled"] = _settings_client.sound.vehicle;
    return value;
}
inline bool Settled(const Event &event)
{
    const std::string_view phase(event.phase);
    return phase == "state_loop" || phase == "update_position" || phase == "call_vehicle_ticks"
        || phase == "autoreplace_queue" || phase == "checkpoint";
}
inline void Capture(const Event &event) noexcept
{
    if (failure != Failure::None || !tracing) return;
    if (sequence >= max_events) {
        failure = Failure::EventBudget;
        sink = nullptr;
        return;
    }
    try {
        Json value = {{"sequence", sequence}, {"action", action}, {"call", call},
            {"phase", event.phase}, {"edge", event.edge}, {"vehicle", event.vehicle},
            {"tile", event.tile}, {"a", event.a}, {"b", event.b}, {"live", Live()}};
        if (event.tile != UINT32_MAX) value["tile_bytes"] = TileBytes(event.tile);
        if (Settled(event)) value["spatial"] = Spatial();
        const std::string line = value.dump();
        if (line.size() + 1 > max_event_bytes - event_bytes) {
            failure = Failure::StorageBudget;
            sink = nullptr;
            return;
        }
        events << line << '\n';
        events.flush();
        if (!events) {
            failure = Failure::Output;
            sink = nullptr;
            return;
        }
        event_bytes += line.size() + 1;
        ++sequence;
    } catch (...) {
        failure = Failure::Snapshot;
        sink = nullptr;
    }
}
class SinkLifetime {
public:
    SinkLifetime(const std::filesystem::path &directory, uint64_t event_limit, uint64_t byte_limit, bool trace)
    {
        ReferenceWorld::Require(sink == nullptr && !events.is_open(), "movement sink already installed");
        ReferenceWorld::Require(event_limit > 0 && event_limit <= 10000000 && byte_limit > 0, "movement observer limits");
        ReferenceWorld::Require(!std::filesystem::exists(directory / "events.jsonl"), "stale movement event stream");
        max_events = event_limit;
        max_event_bytes = byte_limit;
        tracing = trace;
        events.open(directory / "events.jsonl", std::ios::out);
        ReferenceWorld::Require(events.good(), "cannot open movement event stream");
        sink = trace ? Capture : nullptr;
    }
    ~SinkLifetime()
    {
        sink = nullptr;
        events.close();
    }
    SinkLifetime(const SinkLifetime &) = delete;
    SinkLifetime &operator=(const SinkLifetime &) = delete;
};
}
#endif
