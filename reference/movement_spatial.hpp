// SPDX-License-Identifier: GPL-2.0-only
// SOURCE-ONLY DRAFT: include in vehicle.cpp after _vehicles_to_autoreplace definition.
// JSON header/declaration must be supplied by root integration, not by this hook header.
namespace ReferenceMovement {
nlohmann::json Spatial()
{
    nlohmann::json tiles = nlohmann::json::array();
    for (size_t bucket = 0; bucket < _vehicle_tile_hash.size(); ++bucket) {
        nlohmann::json chain = nlohmann::json::array();
        for (const Vehicle *v = _vehicle_tile_hash[bucket]; v != nullptr; v = v->hash_tile_next) {
            if (chain.size() >= Vehicle::GetPoolSize()) throw std::runtime_error("movement observer tile hash cycle");
            chain.push_back({{"id", v->index.base()},
                {"previous_link_valid", v->hash_tile_prev != nullptr && *v->hash_tile_prev == v},
                {"current_bucket_valid", v->hash_tile_current == &_vehicle_tile_hash[bucket]}});
        }
        if (!chain.empty()) tiles.push_back({{"bucket", bucket}, {"chain", chain}});
    }
    nlohmann::json viewport = nlohmann::json::array();
    for (size_t bucket = 0; bucket < _vehicle_viewport_hash.size(); ++bucket) {
        nlohmann::json chain = nlohmann::json::array();
        for (const Vehicle *v = _vehicle_viewport_hash[bucket]; v != nullptr; v = v->hash_viewport_next) {
            if (chain.size() >= Vehicle::GetPoolSize()) throw std::runtime_error("movement observer viewport hash cycle");
            chain.push_back({{"id", v->index.base()},
                {"previous_link_valid", v->hash_viewport_prev != nullptr && *v->hash_viewport_prev == v}});
        }
        if (!chain.empty()) viewport.push_back({{"bucket", bucket}, {"chain", chain}});
    }
    nlohmann::json deferred = nlohmann::json::array();
    for (const auto &[id, restart] : _vehicles_to_autoreplace) deferred.push_back({{"id", id.base()}, {"restart", restart}});
    return {{"tile_hash", tiles}, {"viewport_hash", viewport}, {"autoreplace_queue", deferred}};
}
}
