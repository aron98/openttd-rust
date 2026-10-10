// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ENGINE_SPECS_HPP
#define OTTD_REFERENCE_ENGINE_SPECS_HPP
#include "reference_engine_specs_hooks.hpp"
#include "saveload/reference_content.hpp"
#include <map>
namespace ReferenceEngineSpecs {
using Json = nlohmann::json;
inline bool active = false, used = false, api = false;
inline Json specification, events = Json::array();
inline std::string destination;
inline size_t payload = 0;
[[noreturn]] inline void Refuse(const char *reason) { ReferenceGrfControl::HostError(reason); }
inline uint64_t Unsigned(const Json &value, uint64_t maximum)
{
    if (!value.is_number_unsigned() || value.get<uint64_t>() > maximum) Refuse("engine-spec host unsigned input");
    return value.get<uint64_t>();
}
inline void Keys(const Json &value, std::initializer_list<const char *> keys)
{
    if (!value.is_object() || value.size() != keys.size()) Refuse("engine-spec host schema");
    for (const char *key : keys) if (!value.contains(key)) Refuse("engine-spec host missing field");
}
inline Json FileID(const GRFFile *file) { return file == nullptr ? Json(nullptr) : Json(file->grfid); }
inline Json State()
{
    if (Engine::GetPoolSize() > 512 || _gted.size() > 1024) Refuse("engine-spec host owner capacity");
    Json owners = Json::array(), mapping = Json::array(), temporary = Json::array();
    for (const Engine *engine : Engine::Iterate()) {
        if (!engine->grf_prop.spritegroups.empty() || !engine->overrides.empty()) Refuse("engine-spec host sprite-group scope");
        Json vehicle = nullptr;
        switch (engine->type) {
            case VEH_TRAIN: vehicle = ReferenceContent::Vehicle(engine->VehInfo<RailVehicleInfo>()); break;
            case VEH_ROAD: vehicle = ReferenceContent::Vehicle(engine->VehInfo<RoadVehicleInfo>()); break;
            case VEH_SHIP: vehicle = ReferenceContent::Vehicle(engine->VehInfo<ShipVehicleInfo>()); break;
            case VEH_AIRCRAFT: vehicle = ReferenceContent::Vehicle(engine->VehInfo<AircraftVehicleInfo>()); break;
            default: break;
        }
        Json info = ReferenceContent::Info(engine->info);
        info["string_id"] = engine->info.string_id;
        Json badges = Json::array();
        for (auto badge : engine->badges) badges.push_back(badge.base());
        owners.push_back({{"id", engine->index.base()}, {"type", engine->type}, {"local_id", engine->grf_prop.local_id},
            {"grfid", FileID(engine->grf_prop.grffile)}, {"stored_grfid", engine->grf_prop.grfid}, {"info", info}, {"vehicle", vehicle},
            {"list_position", engine->list_position}, {"original_image_index", engine->original_image_index}, {"badges", badges},
            {"spritegroups", Json::array()}, {"wagon_overrides", Json::array()},
            {"dynamic", {{"name_bytes", std::vector<uint8_t>(engine->name.begin(), engine->name.end())},
                {"intro_date", engine->intro_date.base()}, {"age", engine->age}, {"flags", engine->flags.base()},
                {"reliability", engine->reliability}, {"reliability_spd_dec", engine->reliability_spd_dec},
                {"reliability_start", engine->reliability_start}, {"reliability_max", engine->reliability_max}, {"reliability_final", engine->reliability_final},
                {"duration_phase_1", engine->duration_phase_1}, {"duration_phase_2", engine->duration_phase_2}, {"duration_phase_3", engine->duration_phase_3},
                {"company_avail", engine->company_avail.base()}, {"company_hidden", engine->company_hidden.base()}, {"preview_asked", engine->preview_asked.base()},
                {"preview_company", engine->preview_company.base()}, {"preview_wait", engine->preview_wait},
                {"display_flags", engine->display_flags.base()}, {"display_last_variant", engine->display_last_variant.base()}}}});
    }
    for (size_t type = 0; type < _engine_mngr.mappings.size(); ++type) {
        for (const auto &entry : _engine_mngr.mappings[type]) {
            if (mapping.size() >= 512) Refuse("engine-spec host mapping capacity");
            mapping.push_back({{"bucket", type}, {"type", entry.type}, {"grfid", entry.grfid},
                {"internal_id", entry.internal_id}, {"substitute_id", entry.substitute_id}, {"engine", entry.engine.base()}});
        }
    }
    size_t index = 0;
    for (const auto &entry : _gted) {
        Json rail = Json::array();
        for (auto label : entry.railtypelabels) rail.push_back(label);
        temporary.push_back({{"index", index++}, {"cargo_allowed", entry.cargo_allowed.base()},
            {"cargo_allowed_required", entry.cargo_allowed_required.base()}, {"cargo_disallowed", entry.cargo_disallowed.base()},
            {"railtypelabels", rail}, {"roadtramtype", entry.roadtramtype}, {"defaultcargo_grfid", FileID(entry.defaultcargo_grf)},
            {"refittability", entry.refittability}, {"rv_max_speed", entry.rv_max_speed},
            {"ctt_include_mask", entry.ctt_include_mask}, {"ctt_exclude_mask", entry.ctt_exclude_mask}});
    }
    Json overrides = Json::array();
    for (const auto &[source, target] : _grf_id_overrides) overrides.push_back({source, target});
    return {{"owners", owners}, {"mappings", mapping}, {"temporary", temporary}, {"grfid_overrides", overrides},
        {"pool_capacity", Engine::GetPoolSize()}, {"dynamic_engines", _settings_game.vehicle.dynamic_engines},
        {"context", ReferenceGrfControl::Context()}};
}
inline void Observe(const char *phase, Json detail = nullptr)
{
    if (!active) return;
    if (events.size() >= 96) Refuse("engine-spec host event budget");
    Json row = {{"phase", phase}, {"detail", detail}, {"stage", _cur_gps.stage},
        {"line", _cur_gps.nfo_line}, {"file_grfid", FileID(_cur_gps.grffile)}, {"state", State()}};
    size_t bytes = row.dump().size();
    if (bytes > 1024 * 1024 || payload > 32 * 1024 * 1024 - bytes) Refuse("engine-spec host payload budget");
    payload += bytes;
    events.push_back(std::move(row));
}
inline void CheckCommand(const Json &command)
{
    if (!command.is_object() || !command.contains("op") || !command.at("op").is_string()) Refuse("engine-spec host operation");
    auto op = command.at("op").get<std::string>();
    if (op == "reset-default" || op == "reset-retained" || op == "snapshot") {
        Keys(command, {"op"});
    } else if (op == "acquire") {
        Keys(command, {"op", "grfid", "local_id", "static"});
        Unsigned(command.at("grfid"), UINT32_MAX); Unsigned(command.at("local_id"), UINT16_MAX);
        if (!command.at("static").is_boolean()) Refuse("engine-spec host static flag");
    } else if (op == "property") {
        Keys(command, {"op", "grfid", "first", "count", "property", "raw", "stage"});
        Unsigned(command.at("grfid"), UINT32_MAX); Unsigned(command.at("first"), UINT16_MAX); Unsigned(command.at("count"), 2);
        auto property = Unsigned(command.at("property"), UINT8_MAX);
        if (property != 0x08 && property != 0x09 && property != 0x0F && property != 0x11 && property != 0x01) Refuse("engine-spec host property scope");
        if (!command.at("stage").is_string() || (command.at("stage") != "reserve" && command.at("stage") != "activation")) Refuse("engine-spec host stage");
        if (!command.at("raw").is_array() || command.at("raw").size() > 16) Refuse("engine-spec host raw budget");
        for (const auto &byte : command.at("raw")) Unsigned(byte, UINT8_MAX);
    } else Refuse("engine-spec host unknown operation");
}
inline void Begin(const Json &manifest)
{
    if (manifest.contains("engine_specs") && (std::getenv("OTTD_MOVEMENT_OBSERVE") != nullptr || std::getenv("OTTD_MOVEMENT_PREPARE") != nullptr)) Refuse("engine-spec host mixed movement mode");
    if (!ReferenceGrfControl::active) return;
    const char *path = std::getenv("OTTD_ENGINE_SPECS_OUTPUT");
    if (!manifest.contains("engine_specs")) {
        if (path != nullptr) Refuse("engine-spec host missing mode");
        return;
    }
    if (used || !ReferenceGrfControl::active || path == nullptr || *path == '\0' || std::ifstream(path).good()) Refuse("engine-spec host output admission");
    for (const char *key : {"currency_load", "currency_api", "currency_reload", "currency_owner_encoding", "strings_api"}) {
        if (manifest.contains(key)) Refuse("engine-spec host mixed observer modes");
    }
    specification = manifest.at("engine_specs");
    if (!specification.is_object() || !specification.contains("mode") || !specification.at("mode").is_string()) Refuse("engine-spec host mode");
    api = specification.at("mode") == "api";
    if (api) {
        Keys(specification, {"mode", "dynamic_engines", "commands"});
        if (!manifest.at("files").empty() || !specification.at("commands").is_array() || specification.at("commands").empty() || specification.at("commands").size() > 16) Refuse("engine-spec host API input budget");
        for (const auto &command : specification.at("commands")) CheckCommand(command);
    } else {
        Keys(specification, {"mode", "dynamic_engines"});
        if (specification.at("mode") != "load" || manifest.at("files").size() > 2) Refuse("engine-spec host loader scope");
    }
    if (!specification.at("dynamic_engines").is_boolean()) Refuse("engine-spec host dynamic flag");
    if (!api && specification.at("dynamic_engines").get<bool>() != _settings_game.vehicle.dynamic_engines) Refuse("engine-spec host loader setting mismatch");
    destination = path; used = true; active = true;
    Observe("before-reset");
}
inline void RunApi()
{
    _settings_game.vehicle.dynamic_engines = specification.at("dynamic_engines").get<bool>();
    std::map<uint32_t, GRFFile> fixtures;
    for (const auto &command : specification.at("commands")) {
        auto op = command.at("op").get<std::string>();
        Json result = {{"command", command}};
        if (op == "reset-default") {
            _engine_mngr.ResetToDefaultMapping(); ResetNewGRFData();
        } else if (op == "reset-retained") {
            ResetNewGRFData();
        } else if (op != "snapshot") {
            auto grfid = command.at("grfid").get<uint32_t>();
            if (!fixtures.contains(grfid) && fixtures.size() >= 4) Refuse("engine-spec host fixture budget");
            auto &file = fixtures[grfid]; file.grfid = grfid; file.grf_version = 8;
            GRFFile *previous = _cur_gps.grffile; _cur_gps.grffile = &file;
            if (op == "acquire") {
                auto *engine = GetNewEngine(&file, VEH_ROAD, command.at("local_id").get<uint16_t>(), command.at("static").get<bool>());
                result["engine"] = engine == nullptr ? Json(nullptr) : Json(engine->index.base());
            } else {
                auto raw = command.at("raw").get<std::vector<uint8_t>>();
                ByteReader reader(raw.data(), raw.size());
                uint first = command.at("first").get<uint16_t>(), last = first + command.at("count").get<uint8_t>();
                bool reserve = command.at("stage") == "reserve";
                _cur_gps.stage = reserve ? GLS_RESERVE : GLS_ACTIVATION;
                try {
                    auto status = reserve ? GrfChangeInfoHandler<GSF_ROADVEHICLES>::Reserve(first, last, command.at("property").get<uint8_t>(), reader) :
                        GrfChangeInfoHandler<GSF_ROADVEHICLES>::Activation(first, last, command.at("property").get<uint8_t>(), reader);
                    result["result"] = static_cast<int>(status);
                } catch (const OTTDByteReaderSignal &) { result["read_bounds"] = true; }
                result["remaining"] = reader.Remaining();
            }
            Observe("api-command", result);
            _cur_gps.grffile = previous;
            continue;
        }
        Observe("api-command", result);
    }
    /* Keep fixture GRFFile destinations alive until the dedicated process exits. */
    Observe("api-finish");
    Json result = {{"mode", "original-engine-spec-api-v1"}, {"events", events}, {"baseline_sources", ReferenceGrfControl::baseline_sources},
        {"arms", ReferenceGrfControl::arms}, {"consumptions", 1}, {"input", specification}};
    std::ofstream stream(destination); stream << result.dump() << '\n'; stream.flush();
    if (!stream.good()) Refuse("engine-spec host output failure");
    std::_Exit(0);
}
inline void WriteLoader()
{
    Json result = {{"mode", "actual-loader-engine-spec-v1"}, {"events", events}, {"baseline_sources", ReferenceGrfControl::baseline_sources},
        {"arms", ReferenceGrfControl::arms}, {"consumptions", 1}, {"input", specification}};
    std::ofstream stream(destination); stream << result.dump() << '\n'; stream.flush();
    if (!stream.good()) Refuse("engine-spec host output failure");
    active = false;
}
inline void Finish()
{
    if (!active) return;
    Observe("after-finalize");
    if (api) RunApi();
}
}
void ReferenceEngineSpecsAfterSavedOverlay()
{
    if (!ReferenceEngineSpecs::active || ReferenceEngineSpecs::api) return;
    ReferenceEngineSpecs::Observe("after-saved-engine-overlay");
    ReferenceEngineSpecs::WriteLoader();
}
void ReferenceEngineSpecsRoad(const char *phase, uint first, uint last, int property, size_t remaining, int result, bool unwinding)
{
    ReferenceEngineSpecs::Observe(phase, {{"first", first}, {"last", last}, {"property", property}, {"remaining", remaining}, {"result", result}, {"unwinding", unwinding}});
}
void ReferenceEngineSpecsAllocated(uint local_id, const Engine *engine, size_t remaining)
{
    ReferenceEngineSpecs::Observe("road-owner-resolved", {{"local_id", local_id}, {"engine", engine == nullptr ? nlohmann::json(nullptr) : nlohmann::json(engine->index.base())}, {"remaining", remaining}});
}
ReferenceEngineSpecsRoadScope::ReferenceEngineSpecsRoadScope(uint first, uint last, int property, ByteReader &reader) : first(first), last(last), property(property), reader(reader)
{
    ReferenceEngineSpecsRoad("road-enter", first, last, property, reader.Remaining(), result, false);
}
ReferenceEngineSpecsRoadScope::~ReferenceEngineSpecsRoadScope()
{
    ReferenceEngineSpecsRoad("road-return", first, last, property, reader.Remaining(), result, std::uncaught_exceptions() > exceptions);
}
#endif
