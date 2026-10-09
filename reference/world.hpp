// SPDX-License-Identifier: GPL-2.0-only
// Native-memory observations of the pinned engine's actual save traversal.
#ifndef OTTD_REFERENCE_WORLD_HPP
#define OTTD_REFERENCE_WORLD_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include <fstream>

namespace ReferenceWorld {
using Json = nlohmann::json;
struct Frame {
    std::string name;
    SaveLoadType command;
    Json value;
    std::optional<size_t> count;
};
inline Json state;
inline Json schemas;
inline std::vector<Frame> frames;
inline std::string chunk;
inline uint32_t index;
inline bool raw_chunk = false;
inline bool script = false;
inline bool collecting = false;
inline bool Enabled() { return std::getenv("OTTD_WORLD_PATH") != nullptr; }
inline bool Active() { return collecting && !script; }
bool IsWriting();
inline void Require(bool valid, const char *message)
{
    if (!valid) throw std::runtime_error(message);
}
inline Json Bytes(const void *address, size_t size)
{
    if (size == 0) return Json::array();
    const auto *bytes = static_cast<const uint8_t *>(address);
    return Json(std::vector<uint8_t>(bytes, bytes + size));
}
inline Json Scalar(const void *address, VarType type)
{
    switch (GetVarMemType(type)) {
        case SLE_VAR_BL: return *static_cast<const bool *>(address) ? 1 : 0;
        case SLE_VAR_I8: return *static_cast<const int8_t *>(address);
        case SLE_VAR_U8: return *static_cast<const uint8_t *>(address);
        case SLE_VAR_I16: return *static_cast<const int16_t *>(address);
        case SLE_VAR_U16: return *static_cast<const uint16_t *>(address);
        case SLE_VAR_I32: return *static_cast<const int32_t *>(address);
        case SLE_VAR_U32: return *static_cast<const uint32_t *>(address);
        case SLE_VAR_I64: return *static_cast<const int64_t *>(address);
        case SLE_VAR_U64: return *static_cast<const uint64_t *>(address);
        default: throw std::runtime_error("Unsupported native world scalar");
    }
}
inline void Begin(uint32_t version)
{
    collecting = Enabled();
    if (!collecting) return;
    frames.clear();
    script = false;
    state = {{"schema_version", 1}, {"savegame_version", version}, {"chunks", Json::object()}};
    schemas = Json::object();
}
inline Json Describe(SaveLoadTable table)
{
    Json result = Json::array();
    for (const SaveLoad &field : table) {
        if (!SlIsObjectCurrentlyValid(field.version_from, field.version_to)) continue;
        Json item = {{"name", field.name}, {"command", field.cmd},
            {"file_type", GetVarFileType(field.conv)}, {"memory_type", GetVarMemType(field.conv)},
            // SDT_SSTR uses sizeof(std::string); native string serialization ignores this ABI-specific size.
            {"length", field.cmd == SL_STDSTR ? 0 : field.length}};
        switch (field.cmd) {
            case SL_REF: case SL_REFLIST: case SL_REFVECTOR: item["reference_kind"] = field.conv; break;
            case SL_STRUCT: case SL_STRUCTLIST: item["children"] = Describe(field.handler->GetDescription()); break;
            default: break;
        }
        result.push_back(std::move(item));
    }
    return result;
}
inline void Schema(SaveLoadTable table)
{
    if (collecting && std::getenv("OTTD_WORLD_SCHEMA_PATH") != nullptr && !schemas.contains(chunk)) schemas[chunk] = Describe(table);
}
inline void BeginChunk(uint32_t id, bool riff)
{
    if (!collecting) return;
    Require(frames.empty(), "Unclosed native world frame");
    chunk = std::string{char(id >> 24), char(id >> 16), char(id >> 8), char(id)};
    raw_chunk = riff;
    index = 0;
    Require(!state["chunks"].contains(chunk), "Duplicate native world chunk");
    state["chunks"][chunk] = riff ? Json{{"bytes", Json::array()}} : Json{{"records", Json::object()}};
}
inline void SetIndex(uint32_t value)
{
    if (collecting) index = value;
}
inline void Byte(uint8_t value)
{
    if (!collecting) return;
    if (script) state["chunks"][chunk]["records"][std::to_string(index)]["script_data"].push_back(value);
    else if (raw_chunk) state["chunks"][chunk]["bytes"].push_back(value);
}
inline void EndChunk()
{
    if (!collecting) return;
    Require(frames.empty(), "Unclosed native world object");
    if (raw_chunk) {
        auto &bytes = state["chunks"][chunk]["bytes"];
        Require(bytes.size() >= 4, "Missing native RIFF length");
        bytes.erase(bytes.begin(), bytes.begin() + 4);
    }
    raw_chunk = false;
}
inline void BeginObject()
{
    frames.push_back({{}, SL_NULL, Json::object(), std::nullopt});
}
inline void EndObject()
{
    auto object = std::move(frames.back().value);
    frames.pop_back();
    if (frames.empty()) {
        auto &record = state["chunks"][chunk]["records"][std::to_string(index)];
        if (record.is_null()) record = Json::object();
        for (auto &[name, value] : object.items()) {
            Require(!record.contains(name), "Duplicate native world record field");
            record[name] = std::move(value);
        }
    } else {
        Require(frames.back().command == SL_STRUCT || frames.back().command == SL_STRUCTLIST, "Native child outside struct");
        frames.back().value.push_back(std::move(object));
    }
}
inline void BeginField(const SaveLoad &field)
{
    frames.push_back({field.name, field.cmd, Json::array(), std::nullopt});
}
inline void Value(Json value)
{
    Require(!frames.empty(), "Native value outside field");
    frames.back().value.push_back(std::move(value));
}
inline void Array(const void *address, size_t count, VarType type)
{
    for (size_t i = 0; i < count; ++i) Value(Scalar(static_cast<const char *>(address) + i * SlVarSize(type), type));
}
inline void Count(size_t count)
{
    if (!frames.empty()) frames.back().count = count;
}
inline void EndField()
{
    Frame field = std::move(frames.back());
    frames.pop_back();
    Require(!frames.empty() && frames.back().value.is_object(), "Native field outside object");
    if (field.count) Require(*field.count == field.value.size(), "Native struct/list count mismatch");
    switch (field.command) {
        case SL_VAR: case SL_REF: case SL_STDSTR: case SL_SAVEBYTE:
            Require(field.value.size() == 1, "Native scalar observation count mismatch");
            field.value = std::move(field.value[0]);
            break;
        case SL_ARR: case SL_REFLIST: case SL_REFVECTOR: case SL_DEQUE: case SL_VECTOR:
        case SL_STRUCT: case SL_STRUCTLIST: case SL_NULL: break;
        default: throw std::runtime_error("Unsupported native world descriptor");
    }
    Require(!frames.back().value.contains(field.name), "Duplicate native world field");
    frames.back().value[field.name] = std::move(field.value);
}
inline void BeginScript()
{
    if (!collecting || !IsWriting()) return;
    Require(!script && frames.empty(), "Nested native script observation");
    script = true;
    state["chunks"][chunk]["records"][std::to_string(index)]["script_data"] = Json::array();
}
inline void EndScript() { script = false; }
inline void Finish()
{
    if (!collecting) return;
    Require(frames.empty() && !script, "Unclosed native world observation");
    std::ofstream stream(std::getenv("OTTD_WORLD_PATH"));
    stream.exceptions(std::ios::failbit | std::ios::badbit);
    stream << state.dump() << '\n';
    stream.close();
    if (const char *path = std::getenv("OTTD_WORLD_SCHEMA_PATH")) {
        std::ofstream schema_stream(path);
        schema_stream.exceptions(std::ios::failbit | std::ios::badbit);
        schema_stream << Json{{"schema_version", 1}, {"savegame_version", state["savegame_version"]}, {"chunks", schemas}}.dump() << '\n';
        schema_stream.close();
    }
    collecting = false;
}
}
#endif
