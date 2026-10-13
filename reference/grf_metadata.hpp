// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_METADATA_HPP
#define OTTD_REFERENCE_GRF_METADATA_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../newgrf_config.h"
#include "../newgrf.h"
#include "../newgrf_text.h"
#include "../settings_type.h"
#include "../table/strings.h"
#include <fstream>
namespace ReferenceGrfMetadata {
using Json = nlohmann::json;
inline Json Bytes(std::string_view text) { return std::vector<uint8_t>(text.begin(), text.end()); }
inline Json Texts(const GRFTextList &list)
{
    Json entries = Json::array(), selections = Json::array();
    for (const auto &text : list) entries.push_back({{"language", text.langid}, {"translated", Bytes(text.text)}});
    for (uint8_t language : {0, 1, 2, 127, 255}) {
        SetCurrentGrfLangID(language);
        auto text = GetGRFStringFromGRFText(list);
        selections.push_back({{"language", language}, {"text", text ? Bytes(*text) : Json(nullptr)}});
    }
    return {{"entries", entries}, {"selections", selections}};
}
inline Json Texts(const GRFTextWrapper &wrapper) { return Texts(wrapper ? *wrapper : GRFTextList{}); }
inline void CheckFresh(uint32_t grfid)
{
    for (uint32_t id = 0xD000; id <= 0xFFFF; ++id) {
        if (GetGRFStringID(grfid, GRFStringID{id}) != STR_UNDEFINED) throw std::runtime_error("metadata observer requires fresh string registry");
    }
    for (uint16_t language = 0; language <= 255; ++language) {
        if (LanguageMap::GetLanguageMap(grfid, language) != nullptr) throw std::runtime_error("metadata observer requires absent language map");
    }
}
inline Json Metadata(const GRFConfig &config)
{
    Json compatibility = Json::array();
    for (uint32_t version : {0U, 1U, 2U, 3U, 4U, 6U, 7U, 8U, UINT32_MAX - 1, UINT32_MAX}) {
        compatibility.push_back({{"version", version}, {"compatible", config.IsCompatible(version)}});
    }
    Json parameters = Json::array();
    for (const auto &entry : config.param_info) {
        if (!entry) { parameters.push_back(nullptr); continue; }
        const auto &p = *entry;
        Json labels = Json::array();
        for (const auto &[value, texts] : p.value_names) labels.push_back({{"value", value}, {"texts", Texts(texts)}});
        parameters.push_back({{"slot", p.param_nr}, {"first_bit", p.first_bit}, {"num_bits", p.num_bit}, {"kind", p.type},
            {"min", p.min_value}, {"max", p.max_value}, {"default", p.def_value}, {"complete_labels", p.complete_labels},
            {"name", Texts(p.name)}, {"description", Texts(p.desc)}, {"value_names", labels}});
    }
    return {{"name", Texts(config.name)}, {"description", Texts(config.info)}, {"url", Texts(config.url)},
        {"version", config.version}, {"min_loadable_version", config.min_loadable_version}, {"num_valid_params", config.num_valid_params},
        {"palette_bits", config.palette}, {"has_param_defaults", config.has_param_defaults}, {"parameters", parameters}, {"compatibility", compatibility}};
}
inline Json Values(const GRFConfig &config)
{
    Json reads = Json::array();
    for (const auto &p : config.param_info) reads.push_back(p ? Json(config.GetValue(*p)) : Json(nullptr));
    return {{"parameters", config.param}, {"reads", reads}};
}
inline Json Scan(const Json &scenario)
{
    CheckFresh(0);
    _settings_client.gui.newgrf_default_palette = scenario.at("palette").get<uint8_t>();
    GRFConfig config(scenario.at("path").get<std::string>());
    bool accepted = FillGRFDetails(config, false, NO_DIRECTORY);
    CheckFresh(config.ident.grfid);
    Json failure = nullptr;
    if (!config.errors.empty()) {
        const auto &error = config.errors.front();
        const char *reason = error.message == STR_NEWGRF_ERROR_READ_BOUNDS ? "ReadBounds" : error.message == STR_NEWGRF_ERROR_UNEXPECTED_SPRITE ? "UnexpectedSprite" : "OtherNativeError";
        failure = {{"reason", reason}, {"line", error.nfo_line}};
    }
    Json result = {{"name", scenario.at("name")}, {"accepted", accepted}, {"status", config.status}, {"grfid", config.ident.grfid},
        {"invalid_version", config.flags.Test(GRFConfigFlag::Invalid)}, {"system", config.flags.Test(GRFConfigFlag::System)},
        {"failure", failure}, {"metadata", Metadata(config)}, {"initial", Values(config)}};
    config.SetParameterDefaults();
    result["defaults"] = Values(config);
    config.SetParams(scenario.at("parameters").get<std::vector<uint32_t>>());
    result["supplied"] = Values(config);
    Json writes = Json::array();
    for (const auto &operation : scenario.at("writes")) {
        size_t index = operation.at("index").get<size_t>();
        if (index >= config.param_info.size() || !config.param_info[index]) throw std::runtime_error("invalid metadata operation target");
        config.SetValue(*config.param_info[index], operation.at("value").get<uint32_t>());
        writes.push_back(Values(config));
    }
    result["writes"] = writes;
    return result;
}
inline void Observe()
{
    const char *input = std::getenv("OTTD_GRF_METADATA_MANIFEST"), *output = std::getenv("OTTD_GRF_METADATA_OUTPUT");
    if (input == nullptr || output == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(output).good()) throw std::runtime_error("metadata output already exists");
    std::ifstream stream(input);
    Json manifest = Json::parse(stream), result = {{"scans", Json::array()}, {"texts", Json::array()}};
    auto palette = _settings_client.gui.newgrf_default_palette;
    for (const auto &scenario : manifest.at("scans")) result["scans"].push_back(Scan(scenario));
    for (const auto &scenario : manifest.at("texts")) {
        auto bytes = scenario.at("raw").get<std::vector<uint8_t>>();
        uint32_t grfid = scenario.at("grfid").get<uint32_t>();
        CheckFresh(grfid);
        std::string raw(bytes.begin(), bytes.end());
        result["texts"].push_back({{"name", scenario.at("name")}, {"translated", Bytes(TranslateTTDPatchCodes(grfid, scenario.at("language").get<uint8_t>(), scenario.at("newlines").get<bool>(), raw))}});
    }
    _settings_client.gui.newgrf_default_palette = palette;
    SetCurrentGrfLangID(1);
    std::ofstream destination(output);
    destination << result.dump() << '\n';
    if (!destination) throw std::runtime_error("cannot write metadata observation");
}
}
#endif
