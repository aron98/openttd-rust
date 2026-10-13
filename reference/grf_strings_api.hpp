#ifndef OTTD_REFERENCE_GRF_STRINGS_API_HPP
#define OTTD_REFERENCE_GRF_STRINGS_API_HPP
namespace ReferenceGrfStringsApi {
using Json = nlohmann::json;
Json CompactTable()
{
    Json entries = Json::array();
    for (const auto &entry : _grf_text) {
        Json texts = Json::array();
        for (const auto &text : entry.textholder) {
            texts.push_back({text.langid, std::vector<uint8_t>(text.text.begin(), text.text.end())});
        }
        entries.push_back({entry.grfid, entry.stringid.base(), entry.def_string, texts});
    }
    return {{"selected", _current_lang_id}, {"entries", entries}};
}
[[noreturn]] void Refuse(std::string_view message)
{
    fmt::print(stderr, "GRF strings API fixture refusal: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
Json Run(const Json &commands)
{
    static bool used = false;
    if (used) Refuse("duplicate invocation");
    used = true;
    if (commands.size() > 10000) Refuse("command limit");
    Json initial = CompactTable();
    CleanUpStrings();
    Json results = Json::array();
    size_t payload = 0;
    for (const auto &command : commands) {
        std::string operation = command.at("operation");
        Json result = {{"operation", operation}};
        if (operation == "capacity_fixture") {
            if (!_grf_text.empty()) Refuse("capacity fixture requires empty table");
            uint32_t count = command.at("count").get<uint32_t>();
            if (count > TAB_SIZE_NEWGRF) Refuse("capacity fixture count");
            uint32_t grfid = command.at("grfid").get<uint32_t>();
            if (grfid == 0) Refuse("capacity fixture zero GRFID");
            auto bytes = command.at("raw").get<std::vector<uint8_t>>();
            if (bytes.size() > 16) Refuse("capacity fixture text size");
            std::string text(bytes.begin(), bytes.end());
            for (uint32_t index = 0; index < count; ++index) {
                auto it = _grf_text.emplace(std::end(_grf_text));
                it->grfid = grfid;
                it->stringid = GRFStringID(index);
                it->def_string = command.at("default_id").get<StringID>();
                AddGRFTextToList(it->textholder, command.at("language").get<uint8_t>(), text);
            }
            result["test_only_declared_table"] = command;
        } else if (operation == "define") {
            for (uint16_t language = 0; language < 256; ++language) {
                if (LanguageMap::GetLanguageMap(command.at("grfid").get<uint32_t>(), static_cast<uint8_t>(language)) != nullptr) Refuse("API fixture requires empty defining-file map context");
            }
            const auto bytes = command.at("raw").get<std::vector<uint8_t>>();
            if (bytes.size() > 1024 * 1024) Refuse("input byte limit");
            std::string text(bytes.begin(), bytes.end());
            result["id"] = AddGRFString(command.at("grfid").get<uint32_t>(),
                GRFStringID(command.at("local_id").get<uint32_t>()),
                command.at("language").get<uint8_t>(), command.at("new_scheme").get<bool>(),
                command.at("newlines").get<bool>(), text, command.at("default_id").get<StringID>());
        } else if (operation == "lookup") {
            result["id"] = GetGRFStringID(command.at("grfid").get<uint32_t>(),
                GRFStringID(command.at("local_id").get<uint32_t>()));
        } else if (operation == "select") {
            SetCurrentGrfLangID(command.at("language").get<uint8_t>());
        } else if (operation == "read") {
            StringID id = command.at("id").get<StringID>();
            StringID current = id;
            size_t depth = 0;
            while (true) {
                auto tab = GetStringTab(current);
                if (tab == TEXT_TAB_GAMESCRIPT_START || tab == TEXT_TAB_OLD_NEWGRF) Refuse("unsupported string domain");
                if (tab != TEXT_TAB_NEWGRF_START) break;
                if (++depth > 256) Refuse("default chain budget or cycle");
                auto index = GetStringIndex(current);
                if (index.base() >= _grf_text.size() || _grf_text[index].grfid == 0) Refuse("invalid custom ID");
                if (GetGRFStringFromGRFText(_grf_text[index].textholder).has_value()) break;
                current = _grf_text[index].def_string;
            }
            auto text = GetStringPtr(id);
            result["bytes"] = std::vector<uint8_t>(text.begin(), text.end());
        } else if (operation == "reset") {
            CleanUpStrings();
        } else {
            Refuse("unknown operation");
        }
        result["table"] = CompactTable();
        payload += result.dump().size();
        if (payload > 192 * 1024 * 1024) Refuse("output byte limit");
        results.push_back(std::move(result));
    }
    return {{"initial_table", initial}, {"explicit_fixture_reset", true}, {"results", results}};
}
}
#endif
