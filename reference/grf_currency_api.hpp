#ifndef OTTD_REFERENCE_GRF_CURRENCY_API_HPP
#define OTTD_REFERENCE_GRF_CURRENCY_API_HPP
#include "reference_grf_currency_properties_api.hpp"
namespace ReferenceGrfCurrency {
nlohmann::json RunApi(const nlohmann::json &commands)
{
    if (!commands.is_array() || commands.size() > 4096 || !Pending().empty()) ReferenceGrfControl::HostError("currency API admission");
    nlohmann::json initial = {{"owners", Owners()}, {"strings", ReferenceGrfStringsApi::CompactTable()}};
    CleanUpStrings();
    ResetCurrencies(false);
    nlohmann::json results = nlohmann::json::array();
    size_t bytes = 0;
    for (const auto &command : commands) {
        std::string operation = command.at("operation");
        nlohmann::json result = {{"operation", operation}};
        if (operation == "reset_currencies") {
            ResetCurrencies(command.at("preserve_custom").get<bool>());
        } else if (operation == "custom_fixture") {
            if (byte_owners) ReferenceGrfControl::HostError("text custom fixture in byte mode");
            const auto &input = command.at("owner");
            GetCustomCurrency() = CurrencySpec(input.at("rate").get<uint16_t>(), input.at("separator").get<std::string>(),
                TimerGameCalendar::Year{input.at("to_euro").get<int32_t>()}, input.at("prefix").get<std::string>(),
                input.at("suffix").get<std::string>(), input.at("code").get<std::string>(),
                input.at("symbol_pos").get<uint8_t>(), input.at("name").get<StringID>());
            result["declared_custom_owner"] = input;
        } else if (operation == "custom_bytes") {
            SetCustomBytes(command, result);
        } else if (operation == "property") {
            ApplyProperty(command, result);
        } else if (operation == "reset_strings") {
            CleanUpStrings();
        } else if (operation == "define") {
            uint32_t grfid = command.at("grfid").get<uint32_t>();
            if (grfid == 0) ReferenceGrfControl::HostError("currency API zero string GRFID");
            for (uint16_t language = 0; language < 256; ++language) {
                if (LanguageMap::GetLanguageMap(grfid, static_cast<uint8_t>(language)) != nullptr) ReferenceGrfControl::HostError("currency API requires empty defining maps");
            }
            auto raw = command.at("raw").get<std::vector<uint8_t>>();
            if (raw.size() > 1024 * 1024) ReferenceGrfControl::HostError("currency API string input budget");
            std::string text(raw.begin(), raw.end());
            result["id"] = AddGRFString(grfid, GRFStringID(command.at("local_id").get<uint16_t>()),
                command.at("language").get<uint8_t>(), true, false, text, STR_UNDEFINED);
        } else if (operation == "queue") {
            auto raw = command.at("raw").get<std::vector<uint8_t>>();
            uint first = command.at("first").get<uint16_t>(), count = command.at("count").get<uint8_t>();
            bool reserve = command.at("reserve").get<bool>();
            if (raw.size() > 510) ReferenceGrfControl::HostError("currency API property input budget");
            GRFFile fixture;
            fixture.grfid = command.at("grfid").get<uint32_t>();
            GRFFile *previous = _cur_gps.grffile;
            _cur_gps.grffile = &fixture;
            ByteReader reader(raw.data(), raw.size());
            try {
                auto outcome = reserve ?
                    GrfChangeInfoHandler<GSF_GLOBALVAR>::Reserve(first, first + count, 0x0A, reader) :
                    GrfChangeInfoHandler<GSF_GLOBALVAR>::Activation(first, first + count, 0x0A, reader);
                result["result"] = static_cast<int>(outcome);
            } catch (const OTTDByteReaderSignal &) {
                result["read_bounds"] = true;
            }
            _cur_gps.grffile = previous;
            result["remaining"] = reader.Remaining();
        } else if (operation == "finalize") {
            FinaliseStringMapping();
        } else if (operation == "map") {
            const auto &queries = command.at("queries");
            if (!queries.is_array() || queries.size() > 65536) ReferenceGrfControl::HostError("currency API map query budget");
            result["ids"] = nlohmann::json::array();
            for (const auto &query : queries) result["ids"].push_back(MapGRFStringID(command.at("grfid").get<uint32_t>(), GRFStringID(query.get<uint16_t>())));
        } else if (operation == "read") {
            StringID id = command.at("id").get<StringID>();
            auto table = ReferenceGrfStringsApi::CompactTable();
            if (GetStringTab(id) == TEXT_TAB_GAMESCRIPT_START || GetStringTab(id) == TEXT_TAB_OLD_NEWGRF) ReferenceGrfControl::HostError("unsupported currency API string domain");
            if (GetStringTab(id) == TEXT_TAB_NEWGRF_START && GetStringIndex(id).base() >= table.at("entries").size()) ReferenceGrfControl::HostError("stale custom currency string query");
            auto text = GetStringPtr(id);
            result["bytes"] = std::vector<uint8_t>(text.begin(), text.end());
        } else {
            ReferenceGrfControl::HostError("unknown currency API operation");
        }
        result["owners"] = Owners();
        result["pending"] = Pending();
        result["strings"] = ReferenceGrfStringsApi::CompactTable();
        auto size = result.dump().size();
        if (size > 128 * 1024 * 1024 || bytes > 128 * 1024 * 1024 - size) ReferenceGrfControl::HostError("currency API output budget");
        bytes += size;
        results.push_back(std::move(result));
    }
    if (!Pending().empty()) ReferenceGrfControl::HostError("currency API must finalize queued destinations");
    return {{"initial", initial}, {"explicit_fixture_reset", true}, {"results", results}};
}
}
#endif
