#ifndef OTTD_REFERENCE_TREE_RATING_HPP
#define OTTD_REFERENCE_TREE_RATING_HPP
#include <array>
#include <cstddef>
#include <cstdint>

namespace ReferenceTreeRating {
enum class Kind : uint8_t {
    ScopeEnter, ScopeLeave, Tree, Surface, Suppressed, Applied,
    TestComplete, ExecComplete, ResultComplete,
};
struct Event {
    Kind kind{};
    uint32_t tile = UINT32_MAX;
    uint32_t town = UINT32_MAX;
    uint32_t flags = 0;
    uint32_t company = 0;
    int scope_depth = 0;
    bool test_mode = false;
    size_t map_entries = 0;
    int pass = -1;
    int before_rating = 0;
    int after_rating = 0;
    int saved_rating = 0;
    uint16_t have_ratings = 0;
};
inline std::array<Event, 32768> events{};
inline size_t count = 0;
inline bool enabled = false;
inline bool overflow = false;
inline void Record(const Event &event) noexcept
{
    if (!enabled) return;
    if (count == events.size()) {
        overflow = true;
        return;
    }
    events[count++] = event;
}
inline void Begin() noexcept
{
    count = 0;
    overflow = false;
    enabled = true;
}
inline void End() noexcept { enabled = false; }
}
#endif
