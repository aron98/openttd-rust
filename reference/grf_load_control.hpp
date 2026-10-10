// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_LOAD_CONTROL_HPP
#define OTTD_REFERENCE_GRF_LOAD_CONTROL_HPP
void ReferenceCargoIdentityPreflight(const nlohmann::json &manifest);
namespace ReferenceGrfControl {
using Json = nlohmann::json;
[[noreturn]] inline void HostError(std::string_view message)
{
    fmt::print(stderr, "GRF control observer host refusal: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
inline bool armed = false, active = false, used = false, saved_networking = false;
inline size_t baseset_count = 0, payload_bytes = 0, ignored_loads = 0, ignored_menu_arms = 0, arms = 0;
inline Json manifest, events = Json::array(), current_record;
inline Json before, prepared, baseline_sources = Json::array();
inline std::string output;
inline Json Context()
{
    Json configs = Json::array();
    for (const auto &c : _grfconfig) configs.push_back({{"filename", c->filename}, {"grfid", c->ident.grfid}, {"parameters", c->param}});
    return {{"calendar_date", TimerGameCalendar::date.base()}, {"calendar_year", TimerGameCalendar::year.base()},
        {"calendar_fraction", TimerGameCalendar::date_fract}, {"economy_date", TimerGameEconomy::date.base()},
        {"economy_year", TimerGameEconomy::year.base()}, {"economy_fraction", TimerGameEconomy::date_fract},
        {"tick", TimerGameTick::counter}, {"display", _display_opt}, {"networking", _networking},
        {"random", {_random.state[0], _random.state[1]}}, {"interactive_random", {_interactive_random.state[0], _interactive_random.state[1]}},
        {"configs", configs}};
}
inline const char *Status(GRFStatus status)
{
    switch (status) {
        case GCS_UNKNOWN: return "Unknown";
        case GCS_DISABLED: return "Disabled";
        case GCS_NOT_FOUND: return "NotFound";
        case GCS_INITIALISED: return "Initialised";
        case GCS_ACTIVATED: return "Activated";
    }
    HostError("unknown native GRF status");
}
inline const char *Failure(StringID id)
{
    if (id == STR_NEWGRF_ERROR_READ_BOUNDS) return "ReadBounds";
    if (id == STR_NEWGRF_ERROR_UNEXPECTED_SPRITE) return "UnexpectedSprite";
    if (id == STR_NEWGRF_ERROR_MULTIPLE_ACTION_8) return "MultipleAction8";
    if (id == STR_NEWGRF_ERROR_STATIC_GRF_CAUSES_DESYNC) return "StaticInfluence";
    if (id == STR_NEWGRF_ERROR_TOO_MANY_NEWGRFS_LOADED) return "TooManyFiles";
    if (id == STR_NEWGRF_ERROR_LOAD_AFTER) return "LoadAfter";
    if (manifest.contains("engine_specs") || manifest.contains("cargo_identity")) {
        if (id == STR_NEWGRF_ERROR_UNKNOWN_PROPERTY) return "UnknownProperty";
        if (id == STR_NEWGRF_ERROR_INVALID_ID) return "InvalidID";
    }
    HostError("unclassified native GRF diagnostic");
}
inline Json Files()
{
    Json files = Json::array();
    for (const auto &config : _grfconfig) {
        GRFFile *file = GetFileByFilename(config->filename);
        Json labels = nullptr, parameters = nullptr, errors = Json::array();
        if (file != nullptr) {
            parameters = file->param;
            labels = Json::array();
            for (const auto &label : file->labels) labels.push_back({{"id", label.label}, {"line", label.nfo_line}, {"offset", label.pos}});
        }
        for (const auto &error : config->errors) errors.push_back({{"failure", Failure(error.message)}, {"line", error.nfo_line}});
        files.push_back({{"config_grfid", config->ident.grfid}, {"file_grfid", file ? Json(file->grfid) : Json(nullptr)},
            {"version", file ? Json(file->grf_version) : Json(nullptr)}, {"status", Status(config->status)},
            {"reserved", config->flags.Test(GRFConfigFlag::Reserved)}, {"parameters", parameters}, {"labels", labels}, {"errors", errors}});
    }
    return files;
}
inline Json Overrides()
{
    Json result = Json::array();
    for (const auto &[key, bytes] : _grf_line_to_action6_sprite_override) result.push_back({{"config_grfid", key.grfid}, {"line", key.nfoline}, {"bytes", bytes}});
    return result;
}
inline void Emit(Json event)
{
    payload_bytes += event.dump().size();
    if (events.size() >= 1000000 || payload_bytes > 128 * 1024 * 1024) HostError("native control observer host resource limit");
    events.push_back(std::move(event));
}
inline void Begin(uint num_baseset)
{
    if (!armed) { ++ignored_loads; return; }
    armed = false;
    if (used || active) HostError("duplicate armed GRF load");
    used = true;
    if (num_baseset != 2 || _grfconfig.size() != num_baseset) HostError("control probe requires only original baseline configs");
    const char *path = std::getenv("OTTD_GRF_CONTROL_MANIFEST"), *destination = std::getenv("OTTD_GRF_CONTROL_OUTPUT");
    if (path == nullptr || destination == nullptr) HostError("incomplete control probe environment");
    output = destination;
    if (std::ifstream(output).good()) HostError("control output already exists");
    std::ifstream source(path);
    if (std::getenv("OTTD_ENGINE_SPECS_OUTPUT") != nullptr || std::getenv("OTTD_CARGO_IDENTITY_OUTPUT") != nullptr) {
        source.seekg(0, std::ios::end);
        auto bytes = source.tellg();
        if (bytes < 0 || bytes > 64 * 1024) HostError("engine-spec host manifest byte budget");
        source.seekg(0);
    }
    manifest = Json::parse(source, nullptr, std::getenv("OTTD_CARGO_IDENTITY_OUTPUT") == nullptr);
    if (manifest.is_discarded()) HostError("cargo-identity host invalid JSON");
    ReferenceCargoIdentityPreflight(manifest);
    baseset_count = num_baseset;
    before = Context();
    for (const auto &config : _grfconfig) baseline_sources.push_back(FioFindFullPath(BASESET_DIR, config->filename));
    saved_networking = _networking;
    _networking = manifest.at("networking").get<bool>();
    for (const auto &entry : manifest.at("files")) {
        auto config = std::make_unique<GRFConfig>(entry.at("path").get<std::string>());
        config->ident.grfid = entry.at("grfid").get<uint32_t>();
        config->version = entry.at("metadata_version").get<uint32_t>();
        config->SetParams(entry.at("parameters").get<std::vector<uint32_t>>());
        if (entry.at("static").get<bool>()) config->flags.Set(GRFConfigFlag::Static);
        if (entry.at("init_only").get<bool>()) config->flags.Set(GRFConfigFlag::InitOnly);
        if (entry.at("system").get<bool>()) config->flags.Set(GRFConfigFlag::System);
        _grfconfig.push_back(std::move(config));
    }
    prepared = Context();
    active = true;
}
inline void StageStart(GrfLoadingStage stage)
{
    if (active) Emit({{"kind", "stage_start"}, {"stage", stage}});
}
inline void StageEnd(GrfLoadingStage stage)
{
    if (active) Emit({{"kind", "stage_end"}, {"stage", stage}, {"files", Files()}, {"overrides", Overrides()}});
}
inline void Record(uint32_t length, uint8_t type, uint8_t container)
{
    if (!active) return;
    auto position = std::ranges::find_if(_grfconfig, [](const auto &config) { return config.get() == _cur_gps.grfconfig; });
    if (position == _grfconfig.end()) HostError("missing active probe config");
    current_record = {{"stage", _cur_gps.stage}, {"file", std::distance(_grfconfig.begin(), position)},
        {"line", _cur_gps.nfo_line}, {"offset", _cur_gps.file->GetPos() - (container >= 2 ? 5 : 3)},
        {"action", nullptr}, {"executed", type == 255 && length <= 1024 * 1024 && _cur_gps.skip_sprites == 0}};
}
inline void Action(uint8_t action)
{
    if (active) current_record["action"] = action;
}
inline void Decision()
{
    if (!active) return;
    Json event = current_record;
    event["kind"] = "record";
    event["skip"] = _cur_gps.skip_sprites;
    event["next_line"] = _cur_gps.nfo_line;
    event["next_offset"] = _cur_gps.file->GetPos();
    if (current_record.at("file").get<size_t>() >= baseset_count) {
        event["files"] = Files();
        event["overrides"] = Overrides();
    } else {
        for (size_t index = 0; index < baseset_count; ++index) {
            const auto &config = _grfconfig[index];
            auto *file = GetFileByFilename(config->filename);
            if (file != nullptr && file->grfid != config->ident.grfid) HostError(fmt::format("unsupported baseline dynamic identity changed: {}", config->filename));
        }
    }
    Emit(std::move(event));
}
inline void End()
{
    if (!active) return;
    Json result = {{"phase", "AfterLoadGame-GfxLoadSprites-LoadNewGRF"}, {"baseset_count", baseset_count},
        {"events", events}, {"files", Files()}, {"overrides", Overrides()}, {"before", before}, {"prepared", prepared}, {"after", Context()},
        {"arms", arms}, {"consumptions", 1}, {"ignored_loads", ignored_loads}, {"ignored_menu_arms", ignored_menu_arms}, {"baseline_sources", baseline_sources}};
    std::ofstream destination(output);
    destination << result.dump() << '\n';
    if (!destination) HostError("cannot write control observation");
    _networking = saved_networking;
    active = false;
}
}
void ReferenceArmGrfControl()
{
    if (std::getenv("OTTD_GRF_CONTROL_MANIFEST") == nullptr) return;
    if (_game_mode == GM_MENU) { ++ReferenceGrfControl::ignored_menu_arms; return; }
    if (ReferenceGrfControl::armed || ReferenceGrfControl::used) ReferenceGrfControl::HostError("control observer requires exactly one save load");
    ReferenceGrfControl::armed = true;
    ++ReferenceGrfControl::arms;
}
#endif
