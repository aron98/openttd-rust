#ifndef OTTD_REFERENCE_GRF_CURRENCY_PROPERTIES_API_HPP
#define OTTD_REFERENCE_GRF_CURRENCY_PROPERTIES_API_HPP
namespace ReferenceGrfCurrency {
inline uint64_t PropertyUnsigned(const nlohmann::json &input, const char *key, uint64_t maximum)
{
    if (!input.contains(key) || !input.at(key).is_number_unsigned() || input.at(key).get<uint64_t>() > maximum) ReferenceGrfControl::HostError("currency property unsigned field");
    return input.at(key).get<uint64_t>();
}

inline std::vector<uint8_t> PropertyBytes(const nlohmann::json &input, const char *key, size_t maximum)
{
    if (!input.contains(key) || !input.at(key).is_array() || input.at(key).size() > maximum) ReferenceGrfControl::HostError("currency property byte input budget");
    std::vector<uint8_t> bytes;
    bytes.reserve(input.at(key).size());
    for (const auto &value : input.at(key)) {
        if (!value.is_number_unsigned() || value.get<uint64_t>() > 255) ReferenceGrfControl::HostError("currency property byte value");
        bytes.push_back(value.get<uint8_t>());
    }
    return bytes;
}

inline void ApplyProperty(const nlohmann::json &command, nlohmann::json &result)
{
    if (!byte_owners) ReferenceGrfControl::HostError("currency property requires byte mode");
    auto property = PropertyUnsigned(command, "property", 0x0F);
    if (property < 0x0B) ReferenceGrfControl::HostError("unsupported currency property API");
    uint first = static_cast<uint>(PropertyUnsigned(command, "first", UINT16_MAX));
    uint count = static_cast<uint>(PropertyUnsigned(command, "count", UINT8_MAX));
    uint32_t grfid = static_cast<uint32_t>(PropertyUnsigned(command, "grfid", UINT32_MAX));
    if (!command.contains("reserve") || !command.at("reserve").is_boolean()) ReferenceGrfControl::HostError("currency property stage");
    auto raw = PropertyBytes(command, "raw", 1020);
    GRFFile fixture;
    fixture.grfid = grfid;
    GRFFile *previous = _cur_gps.grffile;
    _cur_gps.grffile = &fixture;
    ByteReader reader(raw.data(), raw.size());
    try {
        auto outcome = command.at("reserve").get<bool>() ?
            GrfChangeInfoHandler<GSF_GLOBALVAR>::Reserve(first, first + count, static_cast<int>(property), reader) :
            GrfChangeInfoHandler<GSF_GLOBALVAR>::Activation(first, first + count, static_cast<int>(property), reader);
        result["result"] = static_cast<int>(outcome);
    } catch (const OTTDByteReaderSignal &) {
        result["read_bounds"] = true;
    }
    _cur_gps.grffile = previous;
    result["remaining"] = reader.Remaining();
    result["property"] = property;
}

inline void SetCustomBytes(const nlohmann::json &command, nlohmann::json &result)
{
    if (!byte_owners) ReferenceGrfControl::HostError("byte custom fixture requires byte mode");
    if (!command.contains("owner") || !command.at("owner").is_object()) ReferenceGrfControl::HostError("currency byte custom owner");
    const auto &input = command.at("owner");
    auto rate = PropertyUnsigned(input, "rate", UINT16_MAX);
    auto position = PropertyUnsigned(input, "symbol_pos", UINT8_MAX);
    auto name = PropertyUnsigned(input, "name", UINT32_MAX);
    if (!input.contains("to_euro") || !input.at("to_euro").is_number_integer() || input.at("to_euro") < INT32_MIN || input.at("to_euro") > INT32_MAX) ReferenceGrfControl::HostError("currency byte custom euro year");
    if (!input.contains("separator") || !input.at("separator").is_string() || input.at("separator").get_ref<const std::string &>().size() > 1020 ||
            !input.contains("code") || !input.at("code").is_string() || input.at("code").get_ref<const std::string &>().size() > 1020) ReferenceGrfControl::HostError("currency byte custom text budget");
    auto prefix = PropertyBytes(input, "prefix", 1020);
    auto suffix = PropertyBytes(input, "suffix", 1020);
    GetCustomCurrency() = CurrencySpec(static_cast<uint16_t>(rate), input.at("separator").get<std::string>(),
        TimerGameCalendar::Year{input.at("to_euro").get<int32_t>()}, std::string(prefix.begin(), prefix.end()),
        std::string(suffix.begin(), suffix.end()), input.at("code").get<std::string>(), static_cast<uint8_t>(position), static_cast<StringID>(name));
    result["declared_custom_owner"] = input;
}
}
#endif
