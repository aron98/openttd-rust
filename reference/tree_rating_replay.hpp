#ifndef OTTD_REFERENCE_TREE_RATING_REPLAY_HPP
#define OTTD_REFERENCE_TREE_RATING_REPLAY_HPP
#include "../reference_tree_rating.hpp"
#include <cstdlib>
#include <string_view>

namespace ReferenceReplay {
inline bool TreeRatingEnabled()
{
    const char *mode = std::getenv("OTTD_TREE_RATING_OBSERVE");
    if (mode == nullptr) return false;
    Require(std::string_view(mode) == "1", "invalid tree rating observer mode");
    for (const char *name : {
        "OTTD_DEPOT_REMOVAL_OBSERVE", "OTTD_ORDERED_SALE_OBSERVE",
        "OTTD_BACKUP_SALE_OBSERVE", "OTTD_BACKUP_ENABLED_SALE_OBSERVE",
        "OTTD_OWNED_RESTORE_OBSERVE", "OTTD_ROAD_SALE_OBSERVE", "OTTD_DEPOT_LIVE",
        "OTTD_ORDER_FIXTURE_PATH", "OTTD_ORDER_NETWORK_INPUT_PATH", "OTTD_WORLD_FIXTURE_MODE",
    }) {
        Require(std::getenv(name) == nullptr, "tree rating and lifecycle observer modes are mutually exclusive");
    }
    return true;
}

inline void TreeRatingPhase(std::string_view phase) noexcept
{
    using namespace ReferenceTreeRating;
    if (!enabled) return;
    if (phase == "test") Record({.kind = Kind::TestComplete});
    else if (phase == "exec") Record({.kind = Kind::ExecComplete});
    else if (phase == "result") Record({.kind = Kind::ResultComplete});
}

inline Json TreeRatingEvent(const ReferenceTreeRating::Event &event)
{
    using ReferenceTreeRating::Kind;
    switch (event.kind) {
        case Kind::ScopeEnter:
        case Kind::ScopeLeave:
            return {{"kind", event.kind == Kind::ScopeEnter ? "scope_enter" : "scope_leave"},
                {"company", event.company}, {"depth", event.scope_depth},
                {"test_mode", event.test_mode}, {"map_entries", event.map_entries}};
        case Kind::Tree:
            return {{"kind", "tree"}, {"tile", event.tile}, {"flags", event.flags}, {"company", event.company}};
        case Kind::Surface:
            return {{"kind", "surface"}, {"tile", event.tile}, {"flags", event.flags},
                {"company", event.company}, {"pass", event.pass}};
        case Kind::Suppressed:
            return {{"kind", "suppressed"}, {"town", event.town}, {"flags", event.flags},
                {"company", event.company}, {"test_mode", event.test_mode}};
        case Kind::Applied:
            return {{"kind", "applied"}, {"town", event.town}, {"flags", event.flags},
                {"company", event.company}, {"test_mode", event.test_mode},
                {"before_rating", event.before_rating}, {"after_rating", event.after_rating},
                {"saved_rating", event.saved_rating}, {"have_ratings", event.have_ratings}};
        case Kind::TestComplete: return {{"kind", "phase_complete"}, {"phase", "test"}};
        case Kind::ExecComplete: return {{"kind", "phase_complete"}, {"phase", "exec"}};
        case Kind::ResultComplete: return {{"kind", "phase_complete"}, {"phase", "result"}};
    }
    throw std::runtime_error("invalid tree rating event kind");
}

class TreeRatingCapture {
    bool active;
public:
    explicit TreeRatingCapture(bool observe) : active(observe)
    {
        if (!active) return;
        Require(!ReferenceTreeRating::enabled, "nested tree rating capture");
        ReferenceTreeRating::Begin();
    }
    TreeRatingCapture(const TreeRatingCapture &) = delete;
    TreeRatingCapture &operator=(const TreeRatingCapture &) = delete;
    ~TreeRatingCapture() { if (active) ReferenceTreeRating::End(); }
    Json Finish()
    {
        Require(active, "tree rating capture is not active");
        ReferenceTreeRating::End();
        active = false;
        Require(!ReferenceTreeRating::overflow, "tree rating event buffer overflow");
        Json trace = Json::array();
        for (size_t ordinal = 0; ordinal < ReferenceTreeRating::count; ++ordinal) {
            Json event = TreeRatingEvent(ReferenceTreeRating::events[ordinal]);
            event["ordinal"] = ordinal;
            trace.push_back(std::move(event));
        }
        return {{"schema_version", 1}, {"events", std::move(trace)}};
    }
};
}
#endif
