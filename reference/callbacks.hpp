// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACKS_HPP
#define OTTD_REFERENCE_CALLBACKS_HPP
#include "reference_callback_vehicle.hpp"
namespace ReferenceCallbacks {
inline void Run(const char *path)
{
    try {
    nlohmann::json output = {{"schema_version", 1}, {"vehicle_cases", ReferenceCallbackVehicle::Run()}};
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
