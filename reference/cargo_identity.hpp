// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CARGO_IDENTITY_HPP
#define OTTD_REFERENCE_CARGO_IDENTITY_HPP
#include "reference_cargo_identity_hooks.hpp"
#include "reference_cargo_identity_admission.hpp"
#include "reference_cargo_identity_state.hpp"
namespace ReferenceCargoIdentity {
inline void Observe(const char *phase, Json detail = nullptr)
{
    if (!active) return;
    if (events.size() >= 96) Refuse("cargo-identity host event budget");
    Json row = {{"phase", phase}, {"detail", detail}, {"stage", _cur_gps.stage},
        {"line", _cur_gps.nfo_line}, {"file", FileIdentity(_cur_gps.grffile)}, {"state", State()}};
    size_t bytes = row.dump().size();
    if (bytes > 1024 * 1024 || payload > 32 * 1024 * 1024 - bytes) Refuse("cargo-identity host payload budget");
    payload += bytes;
    events.push_back(std::move(row));
}
inline void Begin()
{
    if (!requested || !ReferenceGrfControl::active) return;
    if (used || !ReferenceGrfControl::active) Refuse("cargo-identity host lifecycle");
    used = true; active = true;
    Observe("before-reset");
}
inline void Write()
{
    Json result = {{"mode", api ? "original-cargo-identity-api-v1" : "actual-loader-cargo-identity-v1"},
        {"events", events}, {"baseline_sources", ReferenceGrfControl::baseline_sources},
        {"arms", ReferenceGrfControl::arms}, {"consumptions", 1}, {"input", specification}};
    if (std::filesystem::exists(destination)) Refuse("cargo-identity host output already exists");
    std::ofstream stream(destination); stream << result.dump() << '\n'; stream.flush();
    if (!stream.good()) Refuse("cargo-identity host output failure");
    active = false;
}
inline void RunApi()
{
    if (!_gted.empty()) Refuse("cargo-identity host API requires completed temporary release");
    _settings_game.vehicle.dynamic_engines = specification.at("dynamic_engines").get<bool>();
    const auto &commands = specification.at("commands");
    if (commands.at(0).at("op") == "reset-default") _engine_mngr.ResetToDefaultMapping();
    ResetNewGRFData();
    if (!_grf_files.empty()) Refuse("cargo-identity host registry reset failed");
    _grf_files.reserve(specification.at("api_files").size());
    std::vector<std::unique_ptr<GRFConfig>> configs;
    for (const auto &fixture : specification.at("api_files")) {
        auto config = std::make_unique<GRFConfig>(fixture.at("name").get<std::string>());
        config->ident.grfid = fixture.at("grfid").get<uint32_t>();
        InitNewGRFFile(*config);
        _cur_gps.grffile->grf_version = fixture.at("version").get<uint8_t>();
        configs.push_back(std::move(config));
    }
    _cur_gps.grffile = nullptr; _cur_gps.grfconfig = nullptr;
    Observe("api-command", {{"command", commands.at(0)}});
    for (size_t index = 1; index < commands.size(); ++index) {
        const auto &command = commands.at(index);
        auto op = command.at("op").get<std::string>();
        Json result = {{"command", command}};
        if (op == "override") {
            SetNewGRFOverride(command.at("source").get<uint32_t>(), command.at("target").get<uint32_t>());
        } else if (op != "snapshot") {
            auto grfid = command.at("grfid").get<uint32_t>();
            _cur_gps.grffile = GetFileByGRFID(grfid);
            for (auto &config : configs) if (config->ident.grfid == grfid) _cur_gps.grfconfig = config.get();
            if (_cur_gps.grffile == nullptr || _cur_gps.grfconfig == nullptr) Refuse("cargo-identity host missing API registry file");
            if (op == "translate") {
                result["cargo"] = GetCargoTranslation(command.at("cargo").get<uint8_t>(), _cur_gps.grffile, command.at("usebit").get<bool>());
            } else if (op == "build-inverse") {
                BuildCargoTranslationMap();
            } else {
                auto raw = command.at("raw").get<std::vector<uint8_t>>();
                ByteReader reader(raw.data(), raw.size());
                uint feature = command.at("feature").get<uint8_t>(), first = command.at("first").get<uint16_t>();
                uint last = first + command.at("count").get<uint8_t>();
                int property = command.at("property").get<uint8_t>();
                bool reserve = command.at("stage") == "reserve";
                _cur_gps.stage = reserve ? GLS_RESERVE : GLS_ACTIVATION;
                try {
                    ReferenceCargoIdentityScope scope(feature, first, last, property, reader);
                    auto status = Invoke(static_cast<GrfSpecFeature>(feature), reserve, first, last, property, reader);
                    scope.result = static_cast<int>(status); result["result"] = scope.result;
                } catch (const OTTDByteReaderSignal &) { result["read_bounds"] = true; }
                result["remaining"] = reader.Remaining();
            }
        }
        Observe("api-command", result);
        _cur_gps.grffile = nullptr; _cur_gps.grfconfig = nullptr;
    }
    Observe("api-finish");
    Write();
    std::_Exit(0);
}
inline void Finish()
{
    if (!active) return;
    Observe("after-finalize");
    if (api) RunApi();
}
}
void ReferenceCargoIdentityPreflight(const nlohmann::json &manifest)
{
    try { ReferenceCargoIdentity::Preflight(manifest); }
    catch (const std::filesystem::filesystem_error &) { ReferenceCargoIdentity::Refuse("cargo-identity host filesystem input"); }
}
void ReferenceCargoIdentityAfterSavedOverlay()
{
    if (_game_mode != GM_MENU && std::getenv("OTTD_CARGO_IDENTITY_OUTPUT") != nullptr && !ReferenceCargoIdentity::used) ReferenceCargoIdentity::Refuse("cargo-identity host unarmed saved load");
    if (!ReferenceCargoIdentity::active || ReferenceCargoIdentity::api) return;
    ReferenceCargoIdentity::Observe("after-saved-engine-overlay");
    ReferenceCargoIdentity::Write();
}
void ReferenceCargoIdentityProperty(const char *phase, uint feature, uint first, uint last, int property, size_t remaining, int result, bool unwinding)
{
    if (!ReferenceCargoIdentity::active) return;
    if (feature != GSF_CARGOES && !(feature == GSF_GLOBALVAR && property == 9) && feature != GSF_ROADVEHICLES) return;
    ReferenceCargoIdentity::Observe(phase, {{"feature", feature}, {"first", first}, {"last", last},
        {"property", property}, {"remaining", remaining}, {"result", result}, {"unwinding", unwinding}});
}
void ReferenceCargoIdentityAllocated(uint local_id, const Engine *engine, size_t remaining)
{
    if (!ReferenceCargoIdentity::active) return;
    ReferenceCargoIdentity::Observe("road-owner-resolved", {{"local_id", local_id},
        {"engine", engine == nullptr ? nlohmann::json(nullptr) : nlohmann::json(engine->index.base())}, {"remaining", remaining}});
}
ReferenceCargoIdentityScope::ReferenceCargoIdentityScope(uint feature, uint first, uint last, int property, ByteReader &reader) : feature(feature), first(first), last(last), property(property), reader(reader)
{
    ReferenceCargoIdentityProperty("property-enter", feature, first, last, property, reader.Remaining(), result, false);
}
ReferenceCargoIdentityScope::~ReferenceCargoIdentityScope()
{
    ReferenceCargoIdentityProperty("property-return", feature, first, last, property, reader.Remaining(), result, std::uncaught_exceptions() > exceptions);
}
#endif
