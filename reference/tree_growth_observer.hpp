// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_TREE_GROWTH_OBSERVER_HPP
#define OTTD_REFERENCE_TREE_GROWTH_OBSERVER_HPP
#include "tree_map.h"
#include "core/random_func.hpp"
#include "timer/timer_game_tick.h"
#include "3rdparty/nlohmann/json.hpp"
#include <fstream>
extern uint8_t _trees_tick_ctr;
extern TileIndex _cur_tileloop_tile;
namespace ReferenceTreeGrowth {
inline void Emit(const char *phase, const char *edge, uint32_t index)
{
    const char *path = std::getenv("OTTD_TREE_GROWTH_EVENTS");
    if (path == nullptr) return;
    static std::ofstream stream(path, std::ios::out | std::ios::app);
    static uint64_t sequence = 0;
    if (++sequence > 1000000) throw std::runtime_error("tree observation event bound");
    nlohmann::json event = {{"sequence", sequence}, {"phase", phase}, {"edge", edge},
        {"tick", TimerGameTick::counter}, {"random", {_random.state[0], _random.state[1]}},
        {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"trees_tick_ctr", _trees_tick_ctr}, {"cursor", _cur_tileloop_tile.base()}};
    if (index != UINT32_MAX) {
        Tile tile{TileIndex(index)};
        event["tile"] = index;
        event["raw"] = {tile.type(), tile.height(), tile.m1(), tile.m2(), tile.m3(), tile.m4(), tile.m5(), tile.m6(), tile.m7(), tile.m8()};
    }
    stream << event.dump() << '\n';
    if (!stream.good()) throw std::runtime_error("tree observation write failed");
}
class Scope {
    const char *phase;
    uint32_t tile;
public:
    Scope(const char *phase, uint32_t tile = UINT32_MAX) : phase(phase), tile(tile) { Emit(phase, "enter", tile); }
    ~Scope() noexcept(false) { Emit(phase, "leave", tile); }
    Scope(const Scope &) = delete;
    Scope &operator=(const Scope &) = delete;
};
}
#endif
