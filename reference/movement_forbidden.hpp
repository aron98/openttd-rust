#ifndef OTTD_REFERENCE_MOVEMENT_FORBIDDEN_HPP
#define OTTD_REFERENCE_MOVEMENT_FORBIDDEN_HPP
#include <cstdlib>
#include <stdexcept>
#include <string_view>
namespace ReferenceMovement {
inline constexpr const char *forbidden_modes[] = {
    "OTTD_ALLOCATION_PROBES_PATH",
    "OTTD_BACKUP_ENABLED_SALE_OBSERVE",
    "OTTD_BACKUP_SALE_OBSERVE",
    "OTTD_CALLBACK_PROBES_PATH",
    "OTTD_CARGO_IDENTITY_OUTPUT",
    "OTTD_CONTENT_PATH",
    "OTTD_DEPOT_LIVE",
    "OTTD_DEPOT_OCCUPANCY_VECTORS",
    "OTTD_DEPOT_REMOVAL_OBSERVE",
    "OTTD_DEPOT_RUNTIME_PATH",
    "OTTD_DEPOT_RUNTIME_PREPARE",
    "OTTD_DEPOT_RUNTIME_VECTORS",
    "OTTD_ENGINE_SPECS_OUTPUT",
    "OTTD_GAMEPLAY_PROBES_PATH",
    "OTTD_GRF_CONTEXT_OUTPUT",
    "OTTD_GRF_CONTROL_MANIFEST",
    "OTTD_GRF_CONTROL_OUTPUT",
    "OTTD_GRF_CURRENCY_OUTPUT",
    "OTTD_GRF_LANGUAGE_OUTPUT",
    "OTTD_GRF_METADATA_MANIFEST",
    "OTTD_GRF_METADATA_OUTPUT",
    "OTTD_GRF_SAFETY_INPUT",
    "OTTD_GRF_SAFETY_OUTPUT",
    "OTTD_GRF_SCAN_INPUT",
    "OTTD_GRF_SCAN_OUTPUT",
    "OTTD_GRF_SCAN_TRACE",
    "OTTD_GRF_STRINGS_OUTPUT",
    "OTTD_ORDERED_SALE_OBSERVE",
    "OTTD_ORDER_FIXTURE_DIR",
    "OTTD_ORDER_FIXTURE_PATH",
    "OTTD_ORDER_NETWORK_INPUT_PATH",
    "OTTD_ORDER_STATE_PATH",
    "OTTD_OWNED_RESTORE_OBSERVE",
    "OTTD_PRIMITIVES_PATH",
    "OTTD_ROAD_SALE_OBSERVE",
    "OTTD_ROAD_SLOPE_PATH",
    "OTTD_RUNTIME_ROAD_PATH",
    "OTTD_RUNTIME_ROAD_PREPARE",
    "OTTD_SNAPSHOT_PATH",
    "OTTD_TERRAIN_PROBES_PATH",
    "OTTD_TREE_RATING_OBSERVE",
    "OTTD_WORLD_CORRUPT_DERIVED",
    "OTTD_WORLD_DERIVED_PATH",
    "OTTD_WORLD_FIXTURE_MANIFEST_PATH",
    "OTTD_WORLD_FIXTURE_MODE",
    "OTTD_WORLD_PATH",
    "OTTD_WORLD_SCHEMA_PATH",
};
inline bool internal_checkpoint = false;
inline void ValidateEnvironment()
{
    const char *enabled = std::getenv("OTTD_MOVEMENT_OBSERVE");
    if (enabled == nullptr) return;
    if (std::string_view(enabled) != "1") throw std::runtime_error("invalid movement observer mode");
    for (const char *name : forbidden_modes) {
        const std::string_view key(name);
        if (internal_checkpoint && (key == "OTTD_CONTENT_PATH" || key == "OTTD_WORLD_PATH"
                || key == "OTTD_WORLD_SCHEMA_PATH" || key == "OTTD_WORLD_DERIVED_PATH")) continue;
        if (std::getenv(name) != nullptr) throw std::runtime_error("movement observer mode conflict");
    }
}
}
#endif
