#ifndef OTTD_REFERENCE_GRF_STRINGS_HPP
#define OTTD_REFERENCE_GRF_STRINGS_HPP
namespace ReferenceGrfStringsApi {
    nlohmann::json Run(const nlohmann::json &commands);
    nlohmann::json CompactTable();
}
namespace ReferenceGrfStrings {
inline bool active = false, used = false;
inline std::string output;
inline size_t payload = 0;
inline nlohmann::json events = nlohmann::json::array(), initial;
inline nlohmann::json TranslationErrors()
{
    nlohmann::json errors = nlohmann::json::array();
    size_t index = 0;
    for (const auto &config : _grfconfig) {
        if (index >= ReferenceGrfControl::baseset_count) {
            for (const auto &error : config->errors) {
                if (error.message != STR_NEWGRF_ERROR_LOAD_AFTER) continue;
                errors.push_back({{"file", index - ReferenceGrfControl::baseset_count}, {"line", error.nfo_line},
                    {"message", error.message}, {"severity", error.severity}, {"parameters", error.param_value},
                    {"data", std::vector<uint8_t>(error.data.begin(), error.data.end())},
                    {"custom_message", std::vector<uint8_t>(error.custom_message.begin(), error.custom_message.end())}});
            }
        }
        ++index;
    }
    return errors;
}
void TryRun(const nlohmann::json &manifest)
{
    if (!ReferenceGrfControl::active || (!manifest.contains("strings_api") && !manifest.contains("strings_load"))) return;
    if (used) ReferenceGrfControl::HostError("duplicate strings fixture");
    used = true;
    if (manifest.contains("strings_api") && manifest.contains("strings_load")) ReferenceGrfControl::HostError("mixed string fixture modes");
    const char *destination = std::getenv("OTTD_GRF_STRINGS_OUTPUT");
    if (destination == nullptr || *destination == '\0') ReferenceGrfControl::HostError("missing strings API output");
    if (std::ifstream(destination).good()) ReferenceGrfControl::HostError("strings API output exists");
    if (!ReferenceGrfLanguage::active) ReferenceGrfControl::HostError("strings API requires actual language preparation");
    if (manifest.contains("strings_load")) {
        if (!manifest.at("strings_load").get<bool>()) ReferenceGrfControl::HostError("invalid strings load mode");
        output = destination;
        initial = ReferenceGrfStringsApi::CompactTable();
        active = true;
        return;
    }
    auto before = ReferenceGrfControl::Context();
    auto result = ReferenceGrfStringsApi::Run(manifest.at("strings_api"));
    result["mode"] = "separate-process-original-string-api";
    result["catalog"] = ReferenceGrfLanguage::catalog;
    result["context_before"] = before;
    result["context_after"] = ReferenceGrfControl::Context();
    result["arms"] = ReferenceGrfControl::arms;
    result["consumptions"] = 1;
    result["baseline_sources"] = ReferenceGrfControl::baseline_sources;
    if (result["context_after"] != before) ReferenceGrfControl::HostError("strings API mutated observed world context");
    std::ofstream stream(destination);
    stream << result.dump() << '\n';
    stream.flush();
    if (!stream.good()) ReferenceGrfControl::HostError("strings API output write failed");
    stream.close();
}
inline void Observe(nlohmann::json point)
{
    if (!active) return;
    point["table"] = ReferenceGrfStringsApi::CompactTable();
    point["translation_errors"] = TranslationErrors();
    payload += point.dump().size();
    if (events.size() >= 100000 || payload > 128 * 1024 * 1024) ReferenceGrfControl::HostError("strings load trace budget");
    events.push_back(std::move(point));
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
    nlohmann::json result = {{"mode", "actual-stage-major-custom-strings"}, {"events", events},
        {"initial_table", initial}, {"final_table", ReferenceGrfStringsApi::CompactTable()},
        {"translation_errors", TranslationErrors()}, {"arms", ReferenceGrfControl::arms},
        {"consumptions", 1}, {"baseline_sources", ReferenceGrfControl::baseline_sources}};
    std::ofstream stream(output);
    stream << result.dump() << '\n';
    stream.flush();
    if (!stream.good()) ReferenceGrfControl::HostError("strings load output failed");
    active = false;
}
}
#endif
