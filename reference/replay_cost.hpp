// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_COST_HPP
#define OTTD_REFERENCE_REPLAY_COST_HPP
#include "../table/strings.h"
#include "../table/control_codes.h"
#include "../core/utf8.hpp"
#include <charconv>

namespace ReferenceReplay {
inline Json ErrorSymbol(StringID id)
{
    switch (id) {
        case INVALID_STRING_ID: return "CMD_ERROR";
#define REPLAY_ERROR(name) case name: return #name;
        REPLAY_ERROR(STR_ERROR_MAXIMUM_PERMITTED_LOAN)
        REPLAY_ERROR(STR_ERROR_LOAN_ALREADY_REPAID)
        REPLAY_ERROR(STR_ERROR_CURRENCY_REQUIRED)
        REPLAY_ERROR(STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY)
        REPLAY_ERROR(STR_ERROR_OWNED_BY)
        REPLAY_ERROR(STR_ERROR_NAME_MUST_BE_UNIQUE)
        REPLAY_ERROR(STR_ERROR_ALREADY_BUILT)
        REPLAY_ERROR(STR_ERROR_ONEWAY_ROADS_CAN_T_HAVE_JUNCTION)
        REPLAY_ERROR(STR_ERROR_ROAD_WORKS_IN_PROGRESS)
        REPLAY_ERROR(STR_ERROR_LAND_SLOPED_IN_WRONG_DIRECTION)
        REPLAY_ERROR(STR_ERROR_FLAT_LAND_REQUIRED)
        REPLAY_ERROR(STR_ERROR_MUST_DEMOLISH_BRIDGE_FIRST)
        REPLAY_ERROR(STR_ERROR_MUST_REMOVE_ROAD_FIRST)
        REPLAY_ERROR(STR_ERROR_BUILDING_MUST_BE_DEMOLISHED)
        REPLAY_ERROR(STR_ERROR_LOCAL_AUTHORITY_REFUSES_TO_ALLOW_THIS)
        REPLAY_ERROR(STR_ERROR_OBJECT_IN_THE_WAY)
        REPLAY_ERROR(STR_ERROR_CAN_T_CLEAR_THIS_AREA)
        REPLAY_ERROR(STR_ERROR_CLEARING_LIMIT_REACHED)
        REPLAY_ERROR(STR_ERROR_CAN_T_BUILD_ON_WATER)
        REPLAY_ERROR(STR_ERROR_MUST_DEMOLISH_CANAL_FIRST)
#undef REPLAY_ERROR
        default: throw std::runtime_error("unmapped native command error " + std::to_string(id));
    }
}
inline Json Cost(const CommandCost &cost)
{
    Json params = Json::array();
    CommandCost copy = cost;
    const std::string &encoded = copy.GetEncodedMessage().ReferenceEncodedData();
    size_t pos = encoded.find(char(SCC_RECORD_SEPARATOR));
    while (pos != std::string::npos) {
        const size_t end = encoded.find(char(SCC_RECORD_SEPARATOR), pos + 1);
        const std::string_view value(encoded.data() + pos + 1, (end == std::string::npos ? encoded.size() : end) - pos - 1);
        const auto [length, code] = DecodeUtf8(value);
        Require(code == SCC_ENCODED_NUMERIC, "non-numeric command error parameter is outside replay protocol");
        uint64_t number{};
        const auto result = std::from_chars(value.data() + length, value.data() + value.size(), number, 16);
        Require(result.ec == std::errc{} && result.ptr == value.data() + value.size(), "invalid native encoded parameter");
        params.push_back(number);
        pos = end;
    }
    return {{"success", cost.Succeeded()}, {"cost", static_cast<int64_t>(cost.GetCost())},
        {"expenses", static_cast<uint8_t>(cost.GetExpensesType())},
        {"error", cost.Succeeded() ? Json(nullptr) : ErrorSymbol(cost.GetErrorMessage())}, {"error_params", params}};
}
void Phase(const char *phase, const CommandCost &cost)
{
    if (receipt == nullptr) return;
    (*receipt)[phase] = Cost(cost);
    metadata[phase] = {{"error_id", cost.GetErrorMessage()}, {"extra_error_id", cost.GetExtraErrorMessage()},
        {"owner", cost.GetErrorOwner().base()}};
}
void Gate(const char *gate)
{
    if (receipt != nullptr) (*receipt)["gate"] = gate;
}
}
#endif
