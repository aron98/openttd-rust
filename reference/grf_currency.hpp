#ifndef OTTD_REFERENCE_GRF_CURRENCY_HPP
#define OTTD_REFERENCE_GRF_CURRENCY_HPP
namespace ReferenceGrfCurrency {
nlohmann::json Pending();
nlohmann::json RunApi(const nlohmann::json &commands);
inline bool active = false, used = false, reload_armed = false, reload_done = false;
inline bool byte_owners = false;
inline size_t loads_completed = 0;
inline uint8_t current_stage = 0;
inline nlohmann::json specification, loads = nlohmann::json::array(), current_record, reload_context;
inline std::string destination;
inline size_t payload = 0, total_payload = 0;
inline nlohmann::json events = nlohmann::json::array();

inline nlohmann::json Owners()
{
    nlohmann::json rows = nlohmann::json::array();
    for (const auto &owner : _currency_specs) {
        nlohmann::json row = {{"rate", owner.rate}, {"separator", owner.separator}, {"to_euro", owner.to_euro.base()},
            {"code", owner.code}, {"symbol_pos", owner.symbol_pos}, {"name", owner.name}};
        if (byte_owners) {
            row["prefix"] = std::vector<uint8_t>(owner.prefix.begin(), owner.prefix.end());
            row["suffix"] = std::vector<uint8_t>(owner.suffix.begin(), owner.suffix.end());
        } else {
            row["prefix"] = owner.prefix;
            row["suffix"] = owner.suffix;
        }
        rows.push_back(std::move(row));
    }
    return rows;
}

inline void Observe(const char *phase)
{
    if (!active) return;
    nlohmann::json row = {{"phase", phase}, {"stage", current_stage}, {"owners", Owners()}, {"pending", Pending()},
        {"strings", ReferenceGrfStringsApi::CompactTable()}, {"record", current_record}};
    auto size = row.dump().size();
    if (size > 128 * 1024 * 1024 || payload > 128 * 1024 * 1024 - size ||
            total_payload > 256 * 1024 * 1024 - size || events.size() >= 100000) {
        ReferenceGrfControl::HostError("currency observer budget");
    }
    payload += size;
    total_payload += size;
    events.push_back(std::move(row));
}

void MappingAdded() { Observe("mapping-added"); }
void MappingApplied() { Observe("mapping-applied"); }

inline void Write()
{
    nlohmann::json result = {{"mode", "actual-loader-currency-owners"}, {"loads", loads},
        {"reload_context", reload_context}, {"arms", ReferenceGrfControl::arms},
        {"consumptions", loads_completed}, {"baseline_sources", ReferenceGrfControl::baseline_sources}};
    if (byte_owners) result["currency_owner_encoding"] = "bytes-v1";
    std::ofstream stream(destination);
    stream << result.dump() << '\n';
    stream.flush();
    if (!stream.good()) ReferenceGrfControl::HostError("currency output failed");
}
inline void Begin(const nlohmann::json &manifest)
{
    if (reload_armed) {
        if (active || loads_completed != 1) ReferenceGrfControl::HostError("currency reload admission");
        reload_armed = false;
        active = true;
        events = nlohmann::json::array();
        payload = 0;
        current_record = nullptr;
        current_stage = 0;
        Observe("before-reset");
        return;
    }
    if (manifest.contains("currency_owner_encoding")) {
        if (!ReferenceGrfControl::active || (!manifest.contains("currency_load") && !manifest.contains("currency_api"))) ReferenceGrfControl::HostError("currency encoding requires currency fixture");
        if (!manifest.at("currency_owner_encoding").is_string() || manifest.at("currency_owner_encoding") != "bytes-v1") ReferenceGrfControl::HostError("invalid currency owner encoding");
        byte_owners = true;
    }
    if (!ReferenceGrfControl::active || (!manifest.contains("currency_load") && !manifest.contains("currency_api"))) return;
    if (used || manifest.contains("currency_load") == manifest.contains("currency_api")) ReferenceGrfControl::HostError("invalid or duplicate currency fixture");
    if (manifest.contains("strings_api") || !ReferenceGrfLanguage::active) ReferenceGrfControl::HostError("currency fixture context");
    const char *path = std::getenv("OTTD_GRF_CURRENCY_OUTPUT");
    if (path == nullptr || *path == '\0') ReferenceGrfControl::HostError("missing currency output");
    if (std::ifstream(path).good()) ReferenceGrfControl::HostError("currency output exists");
    used = true;
    destination = path;
    specification = manifest;
    if (manifest.contains("currency_api")) {
        if (manifest.contains("currency_reload")) ReferenceGrfControl::HostError("API reload modes mixed");
        auto before = ReferenceGrfControl::Context();
        auto result = RunApi(manifest.at("currency_api"));
        result["mode"] = "separate-process-original-currency-api";
        if (byte_owners) result["currency_owner_encoding"] = "bytes-v1";
        result["arms"] = ReferenceGrfControl::arms;
        result["consumptions"] = 1;
        result["baseline_sources"] = ReferenceGrfControl::baseline_sources;
        result["before"] = before;
        result["after"] = ReferenceGrfControl::Context();
        if (result["after"] != before) ReferenceGrfControl::HostError("currency API changed world context");
        std::ofstream stream(destination);
        stream << result.dump() << '\n';
        stream.flush();
        if (!stream.good()) ReferenceGrfControl::HostError("currency API output failed");
        return;
    }
    if (!manifest.at("currency_load").get<bool>()) ReferenceGrfControl::HostError("invalid currency load mode");
    active = true;
    Observe("before-reset");
}
inline void Record(uint32_t length, uint8_t type, uint8_t container)
{
    if (!active) return;
    current_stage = static_cast<uint8_t>(_cur_gps.stage);
    auto position = std::ranges::find_if(_grfconfig, [](const auto &config) { return config.get() == _cur_gps.grfconfig; });
    if (position == _grfconfig.end()) ReferenceGrfControl::HostError("currency record config absent");
    current_record = {{"stage", _cur_gps.stage}, {"file", std::distance(_grfconfig.begin(), position)},
        {"line", _cur_gps.nfo_line}, {"offset", _cur_gps.file->GetPos() - (container >= 2 ? 5 : 3)},
        {"executed", type == 255 && length <= 1024 * 1024 && _cur_gps.skip_sprites == 0}};
}
inline void StageEnd(GrfLoadingStage stage)
{
    if (!active) return;
    current_stage = static_cast<uint8_t>(stage);
    Observe("stage-end");
}
inline void Finish()
{
    if (!active) return;
    Observe("finish");
    loads.push_back({{"events", events}, {"owners", Owners()}, {"pending", Pending()},
        {"strings", ReferenceGrfStringsApi::CompactTable()}, {"files", ReferenceGrfControl::Files()},
        {"language_files", ReferenceGrfLanguage::Files()}, {"context", ReferenceGrfControl::Context()}});
    ++loads_completed;
    active = false;
    if (!specification.contains("currency_reload")) Write();
}
}

void ReferenceRunGrfCurrencyReload()
{
    using namespace ReferenceGrfCurrency;
    if (!used || !specification.contains("currency_reload")) return;
    if (reload_done || loads_completed != 1 || active) ReferenceGrfControl::HostError("currency reload fixture invocation");
    reload_done = true;
    const auto &order = specification.at("currency_reload");
    if (!order.is_array() || order.size() > _grfconfig.size()) ReferenceGrfControl::HostError("currency reload order size");
    std::vector<bool> selected(_grfconfig.size());
    for (const auto &entry : order) {
        auto index = entry.get<size_t>();
        if (index >= selected.size() || selected[index]) ReferenceGrfControl::HostError("currency reload duplicate or invalid config");
        selected[index] = true;
    }
    reload_context["before"] = ReferenceGrfControl::Context();
    decltype(_grfconfig) next;
    for (const auto &entry : order) next.push_back(std::move(_grfconfig[entry.get<size_t>()]));
    _grfconfig = std::move(next);
    bool networking = _networking;
    _networking = specification.at("networking").get<bool>();
    reload_context["prepared"] = ReferenceGrfControl::Context();
    reload_armed = true;
    GfxLoadSprites();
    if (reload_armed || loads_completed != 2) ReferenceGrfControl::HostError("currency second load not consumed");
    _networking = networking;
    reload_context["after"] = ReferenceGrfControl::Context();
    Write();
}
#include "reference_grf_currency_api.hpp"
#endif
