// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_MOVEMENT_PROTOCOL_HPP
#define OTTD_REFERENCE_MOVEMENT_PROTOCOL_HPP
namespace ReferenceMovement {
inline Json PostRequest(const Json &command)
{
    return {{"company", 0}, {"mode", "post"}, {"command", command}};
}
inline void ProtocolGuard(const Json &protocol)
{
    using ReferenceWorld::Require;
    Require(protocol.is_object(), "movement descriptor object required");
    const std::string mode = protocol.at("mode");
    const bool prepare = mode == "prepare", discover = mode == "discover";
    Require(prepare || discover || mode == "replay", "movement protocol mode");
    Json expected = {{"schema_version", 2}, {"kind", "road_movement"}, {"mode", mode},
        {"trace", protocol.at("trace")}, {"max_calls", prepare ? 512 : discover ? 128 : 256},
        {"max_bytes", prepare ? 805306368 : 402653184}, {"max_seconds", 60}, {"max_events", 200000}};
    Require(protocol.at("trace").is_boolean(), "movement trace boolean required");
    if (prepare) {
        const uint64_t count = Bounded(protocol.at("reservation_count"), 75);
        const Json &commands = protocol.at("commands");
        Require(commands.is_array() && commands.size() == 27, "movement exact construction roster required");
        const uint64_t engine = Bounded(commands.at(26).at("command").at("engine"), UINT16_MAX);
        Require((engine == 116 && (count == 0 || count == 1)) || (engine == 123 && (count == 0 || count == 75)), "movement engine/reservation portfolio");
        Json recipe = Json::array();
        for (uint32_t x = 8; x <= 32; ++x) recipe.push_back(PostRequest({{"kind", "build_road"}, {"tile", 640 + x},
            {"pieces", 10}, {"road_type", 0}, {"toggle_disallowed", 0}, {"town_id", 65535}}));
        recipe.push_back(PostRequest({{"kind", "build_road_depot"}, {"tile", 673}, {"road_type", 0}, {"direction", 0}}));
        recipe.push_back(PostRequest({{"kind", "build_vehicle"}, {"tile", 673}, {"engine", engine},
            {"use_free_vehicles", false}, {"cargo", 255}, {"client_id", 0}}));
        expected["commands"] = recipe;
        expected["reservation_count"] = count;
        expected["prepare_settings"] = Json::array({PostRequest({{"kind", "movement_prepare_original_acceleration"},
            {"name", "vehicle.roadveh_acceleration_model"}, {"value", 0}})});
        expected["witness_tiles"] = Json::array();
        for (uint32_t x = 10; x <= 30; ++x) expected["witness_tiles"].push_back(640 + x);
        Require(protocol.at("trace") == true, "movement preparation trace required");
    } else {
        expected["subject"] = Bounded(protocol.at("subject"), UINT32_MAX - 1);
        if (!discover) expected["calls"] = Bounded(protocol.at("calls"), 256);
    }
    // dump distinguishes boolean, floating-point and integer substitutions too.
    Require(protocol.dump() == expected.dump(), "movement descriptor differs from exact schema2 protocol");
}
}
#endif
