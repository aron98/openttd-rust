#ifndef OTTD_REFERENCE_ROAD_SLOPE_HPP
#define OTTD_REFERENCE_ROAD_SLOPE_HPP
#include "3rdparty/nlohmann/json.hpp"
#include <array>
#include <limits>
#include <stdexcept>

static CommandCost CheckRoadSlope(Slope tileh, RoadBits *pieces, RoadBits existing, RoadBits other);
namespace ReferenceRoadSlope {
using Json = nlohmann::json;
struct Restore {
    bool slopes = _settings_game.construction.build_on_slopes;
    Money foundation = _price[PR_BUILD_FOUNDATION];
    ~Restore()
    {
        _settings_game.construction.build_on_slopes = slopes;
        _price[PR_BUILD_FOUNDATION] = foundation;
    }
};
Json Call(uint8_t slope, uint8_t requested, uint8_t existing, uint8_t other)
{
    RoadBits pieces = static_cast<RoadBits>(requested);
    const CommandCost cost = CheckRoadSlope(static_cast<Slope>(slope), &pieces, static_cast<RoadBits>(existing), static_cast<RoadBits>(other));
    return {{"slope", slope}, {"requested", requested}, {"existing", existing}, {"other", other},
        {"enabled", _settings_game.construction.build_on_slopes}, {"pieces", static_cast<uint8_t>(pieces)},
        {"success", cost.Succeeded()}, {"error_id", cost.GetErrorMessage()},
        {"expenses", static_cast<uint8_t>(cost.GetExpensesType())}, {"cost", static_cast<int64_t>(cost.GetCost())}};
}
Json Probe()
{
    const bool setting = _settings_game.construction.build_on_slopes;
    const Money price = _price[PR_BUILD_FOUNDATION];
    Json rows = Json::array(), prices = Json::array();
    {
        Restore restore;
        const std::array<uint8_t, 19> slopes = {0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,23,27,29,30};
        for (uint8_t slope : slopes) for (uint8_t requested = 0; requested < 16; ++requested)
            for (uint8_t existing = 0; existing < 16; ++existing) for (uint8_t other = 0; other < 16; ++other)
                for (bool enabled : {false, true}) {
                    _settings_game.construction.build_on_slopes = enabled;
                    rows.push_back(Call(slope, requested, existing, other));
                }
        _settings_game.construction.build_on_slopes = true;
        for (int64_t value : {int64_t{0}, int64_t{1}, int64_t{-1}, std::numeric_limits<int64_t>::min(), std::numeric_limits<int64_t>::max()}) {
            _price[PR_BUILD_FOUNDATION] = value;
            prices.push_back({{"price", value}, {"result", Call(1, 1, 0, 0)}});
        }
    }
    if (_settings_game.construction.build_on_slopes != setting || _price[PR_BUILD_FOUNDATION] != price) throw std::runtime_error("Road slope globals leaked");
    return {{"schema_version", 1}, {"foundation_price", static_cast<int64_t>(price)}, {"rows", rows}, {"price_probes", prices},
        {"generic_error_id", CMD_ERROR.GetErrorMessage()}, {"invalid_error_id", INVALID_STRING_ID},
        {"before", {{"build_on_slopes", setting}, {"foundation_price", static_cast<int64_t>(price)}}},
        {"after", {{"build_on_slopes", _settings_game.construction.build_on_slopes}, {"foundation_price", static_cast<int64_t>(_price[PR_BUILD_FOUNDATION])}}}};
}
}
#endif
