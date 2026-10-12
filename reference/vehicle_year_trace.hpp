// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_VEHICLE_YEAR_TRACE_HPP
#define OTTD_REFERENCE_VEHICLE_YEAR_TRACE_HPP
#include "reference_group_profit_snapshot.hpp"
#include <fstream>
namespace ReferenceGroupProfit {
inline void Event(const char *phase, uint32_t vehicle = UINT32_MAX)
{
    if (!Enabled()) return;
    static std::ofstream stream;
    static uint64_t sequence = 0;
    if (!stream.is_open()) {
        stream.exceptions(std::ios::failbit | std::ios::badbit);
        stream.open(std::getenv("OTTD_VEHICLE_YEAR_TRACE"));
    }
    stream << Json{{"sequence", sequence++}, {"phase", phase}, {"vehicle", vehicle},
        {"snapshot", Snapshot()}}.dump() << '\n';
}
}
#endif
