// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_LANGUAGE_HPP
#define OTTD_REFERENCE_GRF_LANGUAGE_HPP
namespace ReferenceGrfLanguageCatalog { nlohmann::json Prepare(const nlohmann::json &input); }
namespace ReferenceGrfLanguageText { nlohmann::json Snapshot(); }
namespace ReferenceGrfLanguage {
using Json = nlohmann::json;
inline bool active = false, used = false;
inline size_t payload = 0;
inline std::string output;
inline Json catalog, specification, events = Json::array(), translated = Json::array(), before, after_prepare;
[[noreturn]] inline void HostError(std::string_view message)
{
    fmt::print(stderr, "GRF language observer host refusal: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
inline Json Files()
{
    Json files = Json::array();
    for (const auto &file : _grf_files) {
        Json maps = Json::object();
        for (const auto &[language, map] : file.language_map) {
            Json genders = Json::array(), cases = Json::array();
            for (const auto &pair : map.gender_map) genders.push_back({pair.newgrf_id, pair.openttd_id});
            for (const auto &pair : map.case_map) cases.push_back({pair.newgrf_id, pair.openttd_id});
            maps[fmt::format("{}", language)] = {{"genders", genders}, {"cases", cases}, {"plural", map.plural_form}};
        }
        uint32_t features = 0;
        for (uint8_t feature = 0; feature < GSF_END; ++feature) {
            if (file.grf_features.Test(static_cast<GrfSpecFeature>(feature))) features |= 1U << feature;
        }
        files.push_back({{"name", file.filename}, {"grfid", file.grfid}, {"features", features}, {"maps", maps}});
    }
    return files;
}
inline void Begin(const Json &manifest)
{
    if (!manifest.contains("language")) return;
    if (used || active) HostError("duplicate fixture load");
    const char *destination = std::getenv("OTTD_GRF_LANGUAGE_OUTPUT");
    if (destination == nullptr) HostError("missing output");
    output = destination;
    if (std::ifstream(output).good()) HostError("output exists");
    used = true;
    specification = manifest.at("language");
    if (specification.at("queries").size() > 1024) HostError("too many translation queries");
    before = ReferenceGrfControl::Context();
    catalog = ReferenceGrfLanguageCatalog::Prepare(specification);
    after_prepare = ReferenceGrfControl::Context();
    if (before != after_prepare) HostError("catalog preparation changed loader world context");
    active = true;
}
inline void Observe(Json point)
{
    if (!active) return;
    point["files"] = Files();
    payload += point.dump().size();
    if (events.size() >= 100000 || payload > 128 * 1024 * 1024) HostError("event budget");
    events.push_back(point);
    for (const auto &query : specification.at("queries")) {
        if (query.at("stage") != point.at("stage") || query.at("file") != point.at("file") || query.at("line") != point.at("line")) continue;
        auto bytes = query.at("raw").get<std::vector<uint8_t>>();
        if (bytes.size() > 1024 * 1024) HostError("translation source budget");
        std::string raw(bytes.begin(), bytes.end());
        Json table_before = ReferenceGrfLanguageText::Snapshot();
        auto value = TranslateTTDPatchCodes(query.at("grfid").get<uint32_t>(), query.at("language").get<uint8_t>(),
            query.at("newlines").get<bool>(), raw);
        Json table_after = ReferenceGrfLanguageText::Snapshot();
        if (table_before != table_after) HostError("translation mutated original string table");
        Json result = {{"query", query.at("id")}, {"stage", point.at("stage")}, {"file", point.at("file")},
            {"line", point.at("line")}, {"bytes", std::vector<uint8_t>(value.begin(), value.end())},
            {"table_before", table_before}, {"table_after", table_after}};
        payload += result.dump().size();
        if (payload > 128 * 1024 * 1024) HostError("translation output budget");
        translated.push_back(std::move(result));
    }
}
inline void Decision()
{
    if (!active) return;
    const auto &record = ReferenceGrfControl::current_record;
    if (record.at("file").get<size_t>() < ReferenceGrfControl::baseset_count) return;
    Observe({{"stage", record.at("stage")}, {"file", record.at("file").get<size_t>() - ReferenceGrfControl::baseset_count},
        {"line", record.at("line")}, {"offset", record.at("offset")}});
}
inline void StageEnd(GrfLoadingStage stage)
{
    if (active) Observe({{"stage", stage}, {"file", 0}, {"line", 0}, {"offset", 0}});
}
inline void Finish()
{
    if (!active) return;
    Json result = {{"phase", "actual-stage-major-language"}, {"catalog", catalog}, {"events", events},
        {"translated", translated}, {"before", before}, {"after_prepare", after_prepare},
        {"after", ReferenceGrfControl::Context()}, {"files", Files()}, {"strings", ReferenceGrfLanguageText::Snapshot()}};
    std::ofstream destination(output);
    destination << result.dump() << '\n';
    if (!destination) HostError("cannot write observation");
    active = false;
}
}
#endif
