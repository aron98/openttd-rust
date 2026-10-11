// SPDX-License-Identifier: GPL-2.0-only
// SOURCE-ONLY DRAFT: root must integrate and compile before native use.
#ifndef OTTD_REFERENCE_MOVEMENT_HOOKS_HPP
#define OTTD_REFERENCE_MOVEMENT_HOOKS_HPP
#include <cstdint>
#include <cstddef>
namespace ReferenceMovement {
struct Event {
    const char *phase;
    const char *edge;
    uint32_t vehicle;
    uint32_t tile;
    int64_t a;
    int64_t b;
};
using Sink = void (*)(const Event &);
inline Sink sink = nullptr;
struct RouteEvent {
    const char *phase;
    uint32_t tile;
    int track;
    uint32_t end_tile;
    int end_track;
    uint32_t parent_tile;
    int parent_track;
    int cost;
    int estimate;
    size_t slot;
    size_t count;
};
using RouteSink = void (*)(const RouteEvent &);
inline RouteSink route_sink = nullptr;
inline bool routing_active = false;
class RoutingScope {
    bool previous;
public:
    RoutingScope() : previous(routing_active) { routing_active = route_sink != nullptr; }
    ~RoutingScope() { routing_active = previous; }
    RoutingScope(const RoutingScope &) = delete;
    RoutingScope &operator=(const RoutingScope &) = delete;
};
template <typename Node>
inline void RouteNode(const char *phase, const Node &node, size_t slot = 0, size_t count = 0)
{
    if constexpr (requires { node.segment_last_tile; node.segment_last_td; }) {
        if (!routing_active || route_sink == nullptr) return;
        route_sink({phase, node.GetTile().base(), static_cast<int>(node.GetTrackdir()),
            node.segment_last_tile.base(), static_cast<int>(node.segment_last_td),
            node.parent == nullptr ? UINT32_MAX : node.parent->GetTile().base(),
            node.parent == nullptr ? -1 : static_cast<int>(node.parent->GetTrackdir()),
            node.cost, node.estimate, slot, count});
    }
}
template <typename Items>
inline void RouteHeap(const char *phase, const Items &items, size_t count)
{
    if (!routing_active || route_sink == nullptr) return;
    route_sink({phase, UINT32_MAX, -1, UINT32_MAX, -1, UINT32_MAX, -1, 0, 0, 0, count});
    for (size_t slot = 1; slot <= count; ++slot) RouteNode(phase, *items[slot], slot, count);
}
inline void Emit(const char *phase, const char *edge, uint32_t vehicle = UINT32_MAX,
        uint32_t tile = UINT32_MAX, int64_t a = 0, int64_t b = 0) noexcept
{
    if (sink != nullptr) sink({phase, edge, vehicle, tile, a, b});
}
inline bool Result(const char *phase, uint32_t vehicle, bool value) noexcept
{
    Emit(phase, "result", vehicle, UINT32_MAX, value);
    return value;
}
class Scope {
    Event event;
public:
    Scope(const char *phase, uint32_t vehicle = UINT32_MAX,
            uint32_t tile = UINT32_MAX, int64_t a = 0, int64_t b = 0) noexcept :
        event{phase, "leave", vehicle, tile, a, b}
    {
        Emit(phase, "enter", vehicle, tile, a, b);
    }
    ~Scope() { if (sink != nullptr) sink(event); }
    Scope(const Scope &) = delete;
    Scope &operator=(const Scope &) = delete;
};
}
#endif
