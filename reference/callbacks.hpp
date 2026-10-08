// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACKS_HPP
#define OTTD_REFERENCE_CALLBACKS_HPP
#include "reference_callback_vehicle.hpp"
#include "reference_callback_house.hpp"
#include "reference_callback_company.hpp"
#include "reference_callback_station.hpp"
#include "reference_callback_industry.hpp"
namespace ReferenceCallbacks {
inline void Run(const char *path)
{
    try {
    auto original_timers = TimerManager<TimerGameEconomy>::GetTimers();
    nlohmann::json output = {{"schema_version", 1}, {"vehicle_cases", ReferenceCallbackVehicle::Run()}};
    TimerManager<TimerGameEconomy>::GetTimers() = original_timers;
    output["periodic_cases"] = nlohmann::json::array();
    for (auto cases : {ReferenceCallbackHouse::Run(), ReferenceCallbackCompany::Run(), ReferenceCallbackStation::Run()}) {
        for (auto &entry : cases) output["periodic_cases"].push_back(std::move(entry));
    }
    output["industry_cases"] = ReferenceCallbackIndustry::Run();
    std::ofstream stream(path);
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << output.dump() << '\n';
    stream.close();
    std::_Exit(EXIT_SUCCESS);
    } catch (const std::exception &error) {
        fprintf(stderr, "Callback probe failed: %s\n", error.what());
        std::_Exit(EXIT_FAILURE);
    }
}
}
#endif
