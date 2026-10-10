// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CARGO_IDENTITY_ADMISSION_HPP
#define OTTD_REFERENCE_CARGO_IDENTITY_ADMISSION_HPP
#include <filesystem>
#include <set>
namespace ReferenceCargoIdentity {
using Json = nlohmann::json;
inline bool requested = false, active = false, used = false, api = false;
inline Json specification, events = Json::array();
inline std::string destination;
inline size_t payload = 0;
[[noreturn]] inline void Refuse(const char *reason) { ReferenceGrfControl::HostError(reason); }
inline uint64_t Unsigned(const Json &value, uint64_t maximum)
{
    if (!value.is_number_unsigned() || value.get<uint64_t>() > maximum) Refuse("cargo-identity host unsigned input");
    return value.get<uint64_t>();
}
inline void Keys(const Json &value, std::initializer_list<const char *> keys)
{
    if (!value.is_object() || value.size() != keys.size()) Refuse("cargo-identity host schema");
    for (const char *key : keys) if (!value.contains(key)) Refuse("cargo-identity host missing field");
}
inline void Boolean(const Json &value)
{
    if (!value.is_boolean()) Refuse("cargo-identity host boolean");
}
inline std::string String(const Json &value)
{
    if (!value.is_string()) Refuse("cargo-identity host string");
    auto result = value.get<std::string>();
    if (result.find('\0') != std::string::npos) Refuse("cargo-identity host embedded NUL");
    return result;
}
inline void CheckCommand(const Json &command, size_t index, const std::set<uint32_t> &ids)
{
    if (!command.is_object() || !command.contains("op")) Refuse("cargo-identity host operation");
    auto op = String(command.at("op"));
    if (index == 0) {
        Keys(command, {"op"});
        if (op != "reset-default" && op != "reset-retained") Refuse("cargo-identity host initial reset");
        return;
    }
    if (op == "snapshot") { Keys(command, {"op"}); return; }
    if (op == "override") {
        Keys(command, {"op", "source", "target"});
        auto source = Unsigned(command.at("source"), UINT32_MAX);
        Unsigned(command.at("target"), UINT32_MAX);
        if (!ids.contains(source)) Refuse("cargo-identity host missing source file");
        return;
    }
    if (op == "property") {
        Keys(command, {"op", "feature", "stage", "grfid", "first", "count", "property", "raw"});
        auto feature = Unsigned(command.at("feature"), UINT8_MAX), property = Unsigned(command.at("property"), UINT8_MAX);
        if (!((feature == 11 && (property == 8 || property == 0x17 || property == 1)) ||
            (feature == 8 && property == 9) || (feature == 1 && (property == 0x10 || property == 8)))) Refuse("cargo-identity host property scope");
        auto stage = String(command.at("stage"));
        if (stage != "reserve" && stage != "activation") Refuse("cargo-identity host stage");
        Unsigned(command.at("first"), UINT16_MAX); Unsigned(command.at("count"), 2);
        if (!command.at("raw").is_array() || command.at("raw").size() > 1020) Refuse("cargo-identity host raw budget");
        for (const auto &byte : command.at("raw")) Unsigned(byte, UINT8_MAX);
    } else if (op == "translate") {
        Keys(command, {"op", "grfid", "cargo", "usebit"});
        Unsigned(command.at("cargo"), UINT8_MAX); Boolean(command.at("usebit"));
    } else if (op == "build-inverse") {
        Keys(command, {"op", "grfid"});
    } else Refuse("cargo-identity host unknown operation or unsafe reset");
    auto grfid = Unsigned(command.at("grfid"), UINT32_MAX);
    if (!ids.contains(grfid)) Refuse("cargo-identity host missing file");
}
inline void Preflight(const Json &manifest)
{
    const char *path = std::getenv("OTTD_CARGO_IDENTITY_OUTPUT");
    if (!manifest.contains("cargo_identity") && path == nullptr) return;
    Keys(manifest, {"networking", "files", "cargo_identity"});
    if (requested || used || path == nullptr || *path == '\0' || !std::filesystem::path(path).is_absolute() || !std::filesystem::is_directory(std::filesystem::path(path).parent_path()) || std::filesystem::exists(path) || std::filesystem::is_symlink(path)) Refuse("cargo-identity host output admission");
    for (const char *key : {"OTTD_ENGINE_SPECS_OUTPUT", "OTTD_MOVEMENT_OBSERVE", "OTTD_MOVEMENT_PREPARE", "OTTD_REPLAY_PATH", "OTTD_GRF_CURRENCY_OUTPUT", "OTTD_GRF_STRINGS_OUTPUT"}) {
        if (std::getenv(key) != nullptr) Refuse("cargo-identity host mixed environment");
    }
    Boolean(manifest.at("networking"));
    if (manifest.at("networking").get<bool>()) Refuse("cargo-identity host networking scope");
    const auto &files = manifest.at("files");
    if (!files.is_array() || files.size() > 2) Refuse("cargo-identity host configured file budget");
    std::set<uint32_t> configured_ids;
    std::set<std::string> configured_paths;
    for (const auto &file : files) {
        Keys(file, {"path", "grfid", "metadata_version", "parameters", "static", "init_only", "system"});
        auto filename = String(file.at("path"));
        if (!std::filesystem::path(filename).is_absolute() || !std::filesystem::is_regular_file(filename) ||
            std::filesystem::file_size(filename) > 4096 || !configured_paths.insert(filename).second) Refuse("cargo-identity host configured file");
        auto grfid = Unsigned(file.at("grfid"), UINT32_MAX);
        if (!configured_ids.insert(grfid).second) Refuse("cargo-identity host duplicate GRFID");
        Unsigned(file.at("metadata_version"), UINT32_MAX);
        if (!file.at("parameters").is_array() || file.at("parameters").size() > 128) Refuse("cargo-identity host parameter budget");
        for (const auto &value : file.at("parameters")) Unsigned(value, UINT32_MAX);
        for (const char *flag : {"static", "init_only", "system"}) Boolean(file.at(flag));
    }
    specification = manifest.at("cargo_identity");
    if (!specification.is_object() || !specification.contains("mode")) Refuse("cargo-identity host mode");
    auto mode = String(specification.at("mode"));
    api = mode == "api";
    if (api) {
        Keys(specification, {"mode", "dynamic_engines", "api_files", "commands"});
        const auto &fixtures = specification.at("api_files"), &commands = specification.at("commands");
        if (!files.empty() || !fixtures.is_array() || fixtures.empty() || fixtures.size() > 4 ||
            !commands.is_array() || commands.empty() || commands.size() > 16) Refuse("cargo-identity host API budget");
        std::set<uint32_t> ids;
        std::set<std::string> names;
        for (const auto &fixture : fixtures) {
            Keys(fixture, {"grfid", "version", "name"});
            auto id = Unsigned(fixture.at("grfid"), UINT32_MAX), version = Unsigned(fixture.at("version"), 8);
            auto name = String(fixture.at("name"));
            if (version < 6 || !ids.insert(id).second || name.empty() || name.size() > 64 ||
                name.find_first_of("/\\") != std::string::npos || !names.insert(name).second) Refuse("cargo-identity host API file");
        }
        size_t index = 0;
        for (const auto &command : commands) CheckCommand(command, index++, ids);
    } else {
        Keys(specification, {"mode", "dynamic_engines"});
        if (mode != "load") Refuse("cargo-identity host mode");
    }
    Boolean(specification.at("dynamic_engines"));
    if (!api && specification.at("dynamic_engines").get<bool>() != _settings_game.vehicle.dynamic_engines) Refuse("cargo-identity host loader setting mismatch");
    destination = path; requested = true;
}
}
#endif
