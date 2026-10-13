// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_SAFETY_HPP
#define OTTD_REFERENCE_GRF_SAFETY_HPP
namespace ReferenceGrfSafety {
using Json = nlohmann::json;
static bool active = false;
static Json decisions;
static Json *current = nullptr;
[[noreturn]] static void HostError(const char *message)
{
    fmt::print(stderr, "GRF safety observer: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
struct Record {
    bool observing;
    Record(GrfLoadingStage stage, uint8_t version) : observing(active && stage == GLS_SAFETYSCAN)
    {
        if (!observing) return;
        if (decisions.size() >= 1000000) HostError("record budget");
        decisions.push_back({{"line", _cur_gps.nfo_line}, {"offset", _cur_gps.file->GetPos() - (version == 1 ? 3 : 5)},
            {"action", nullptr}, {"consumed", 0}, {"skip", _cur_gps.skip_sprites}});
        current = &decisions.back();
    }
    ~Record()
    {
        if (!observing) return;
        (*current)["skip"] = _cur_gps.skip_sprites;
        current = nullptr;
    }
};
static void Decoded(GrfLoadingStage stage, uint8_t action, size_t consumed)
{
    if (!active || stage != GLS_SAFETYSCAN || current == nullptr) return;
    (*current)["action"] = action;
    (*current)["consumed"] = consumed;
}
static Json Error(const GRFConfig &config)
{
    if (config.errors.empty()) return nullptr;
    const auto &error = config.errors.front();
    const char *reason = error.message == STR_NEWGRF_ERROR_READ_BOUNDS ? "ReadBounds" :
        error.message == STR_NEWGRF_ERROR_UNEXPECTED_SPRITE ? "UnexpectedSprite" : "OtherNativeError";
    return {{"reason", reason}, {"line", error.nfo_line}};
}
void Observe()
{
    const char *input = std::getenv("OTTD_GRF_SAFETY_INPUT");
    const char *output = std::getenv("OTTD_GRF_SAFETY_OUTPUT");
    if (input == nullptr && output == nullptr) return;
    if (_game_mode == GM_MENU) return;
    if (input == nullptr || output == nullptr || _game_mode != GM_NORMAL) HostError("dedicated saved-game invocation required");
    if (std::getenv("OTTD_REPLAY_PATH") != nullptr) HostError("replay forbidden");
    if (std::ifstream(output).good()) HostError("output already exists");
    if (!_grf_line_to_action6_sprite_override.empty()) HostError("requires empty substitution registry");
    std::ifstream stream(input);
    Json manifest;
    stream >> manifest;
    if (!stream || !manifest.is_array() || manifest.size() > 10000) HostError("invalid manifest");
    Json before = ReferenceGrfControl::Context();
    Json files_before = ReferenceGrfControl::Files();
    auto saved_gps = _cur_gps;
    GRFConfigList saved_configs;
    saved_configs.swap(_grfconfig);
    Json results = Json::array();
    for (const auto &scenario : manifest) {
        _grfconfig.clear();
        for (const auto &entry : scenario.at("configs")) {
            auto config = std::make_unique<GRFConfig>();
            config->ident.grfid = entry.at("grfid").get<uint32_t>();
            if (entry.at("is_static").get<bool>()) config->flags.Set(GRFConfigFlag::Static);
            _grfconfig.push_back(std::move(config));
        }
        _cur_gps.grffile = nullptr;
        decisions = Json::array();
        GRFConfig config(scenario.at("path").get<std::string>());
        active = true;
        bool accepted = FillGRFDetails(config, scenario.at("is_static").get<bool>(), NO_DIRECTORY);
        active = false;
        Json identity = accepted ? Json{{"grfid", config.ident.grfid}, {"md5", config.ident.md5sum}} : Json(nullptr);
        Json name = nullptr, info = nullptr;
        if (auto value = GetGRFStringFromGRFText(config.name); value.has_value()) name = std::vector<uint8_t>(value->begin(), value->end());
        if (auto value = GetGRFStringFromGRFText(config.info); value.has_value()) info = std::vector<uint8_t>(value->begin(), value->end());
        results.push_back({{"id", scenario.at("id")}, {"accepted", accepted}, {"identity", identity},
            {"grfid", config.ident.grfid}, {"status", config.status}, {"unsafe", config.flags.Test(GRFConfigFlag::Unsafe)},
            {"system", config.flags.Test(GRFConfigFlag::System)}, {"invalid", config.flags.Test(GRFConfigFlag::Invalid)},
            {"name", name}, {"info", info}, {"failure", Error(config)}, {"decisions", decisions}});
    }
    _grfconfig.clear();
    saved_configs.swap(_grfconfig);
    _cur_gps = std::move(saved_gps);
    Json restored = ReferenceGrfControl::Context();
    Json files_restored = ReferenceGrfControl::Files();
    if (before != restored || files_before != files_restored) HostError("fixture changed original context or files");
    std::ofstream target(output);
    target << Json{{"scans", results}, {"before", before}, {"restored", restored},
        {"files_before", files_before}, {"files_restored", files_restored}}.dump() << '\n';
    if (!target) HostError("cannot write output");
}
}
#endif
