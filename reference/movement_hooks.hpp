// SPDX-License-Identifier: GPL-2.0-only
// SOURCE-ONLY DRAFT: root must integrate and compile before native use.
#ifndef OTTD_REFERENCE_MOVEMENT_HOOKS_HPP
#define OTTD_REFERENCE_MOVEMENT_HOOKS_HPP
#include <cstdint>
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
