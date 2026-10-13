// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_COMPANY_RATING_TRACE_HPP
#define OTTD_REFERENCE_COMPANY_RATING_TRACE_HPP
#include "reference_company_rating_snapshot.hpp"
#include <fstream>
namespace ReferenceCompanyRating {
inline void Event(const char *phase, CompanyID owner = CompanyID::Invalid())
{
    const char *path = std::getenv("OTTD_COMPANY_RATING_TRACE");
    if (path == nullptr) return;
    static std::ofstream stream;
    static uint64_t sequence = 0;
    if (!stream.is_open()) {
        stream.exceptions(std::ios::failbit | std::ios::badbit);
        stream.open(path);
    }
    stream << Json{{"sequence", sequence++}, {"phase", phase}, {"owner", owner.base()},
        {"snapshot", Snapshot()}}.dump() << '\n';
}
}
#endif
