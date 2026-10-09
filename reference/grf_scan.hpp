// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_SCAN_HPP
#define OTTD_REFERENCE_GRF_SCAN_HPP
#include "../3rdparty/nlohmann/json.hpp"
#include "../newgrf_config.h"
#include "../newgrf.h"
#include "../newgrf_text.h"
#include "../spriteloader/sprite_file_type.hpp"
#include "../fileio_func.h"
#include "../spritecache.h"
#include "../table/strings.h"
#include <fstream>
namespace ReferenceGrfScan {
using Json = nlohmann::json;
inline Json RecordTrace(const std::string &path)
{
    SpriteFile file(path, NO_DIRECTORY, false);
    uint8_t version = file.GetContainerVersion();
    if (version == 0) throw std::runtime_error("invalid trace container");
    size_t sprite_section = 0;
    if (version == 2) {
        sprite_section = 14 + file.ReadDword();
        if (file.ReadByte() != 0) throw std::runtime_error("unsupported trace compression");
    }
    auto read_length = [&]() -> uint32_t { return version == 1 ? file.ReadWord() : file.ReadDword(); };
    if (read_length() != 4 || file.ReadByte() != 255) throw std::runtime_error("invalid trace count record");
    file.ReadDword();
    Json records = Json::array();
    for (;;) {
        size_t start = file.GetPos();
        uint32_t length = read_length();
        if (length == 0) break;
        uint8_t type = file.ReadByte();
        if (type == 255 || (version == 2 && type == 253)) {
            file.SkipBytes(length);
        } else {
            if (length < 8 || length > UINT16_MAX) throw std::runtime_error("invalid inline trace length");
            file.SkipBytes(7);
            if (!SkipSpriteData(file, type, length - 8)) throw std::runtime_error("invalid native sprite data");
        }
        records.push_back({{"start", start}, {"end", file.GetPos()}, {"length", length}, {"type", type}});
        if (records.size() > 100000) throw std::runtime_error("trace record limit");
    }
    Json sprites = Json::array();
    if (version == 2) {
        file.SeekTo(sprite_section, SEEK_SET);
        for (;;) {
            size_t start = file.GetPos();
            uint32_t id = file.ReadDword();
            if (id == 0) break;
            uint32_t length = file.ReadDword();
            file.SkipBytes(length);
            sprites.push_back({{"id", id}, {"start", start}, {"end", file.GetPos()}, {"length", length}});
            if (sprites.size() > 100000) throw std::runtime_error("trace sprite limit");
        }
    }
    return {{"version", version}, {"records", records}, {"sprites", sprites}};
}
inline void Observe()
{
    const char *input = std::getenv("OTTD_GRF_SCAN_INPUT");
    const char *output = std::getenv("OTTD_GRF_SCAN_OUTPUT");
    if (input == nullptr || output == nullptr || _game_mode == GM_MENU) return;
    if (std::ifstream(output).good()) throw std::runtime_error("GRF scan output already exists");
    GRFConfig config(input);
    bool accepted = FillGRFDetails(config, false, NO_DIRECTORY);
    Json identity = nullptr;
    if (accepted) identity = {{"grfid", config.ident.grfid}, {"md5", config.ident.md5sum}};
    Json result = {{"accepted", accepted}, {"status", config.status}, {"grfid", config.ident.grfid},
        {"identity", identity}, {"invalid_version", config.flags.Test(GRFConfigFlag::Invalid)},
        {"system", config.flags.Test(GRFConfigFlag::System)}};
    if (auto name = GetGRFStringFromGRFText(config.name); name.has_value()) result["name"] = *name;
    if (auto info = GetGRFStringFromGRFText(config.info); info.has_value()) result["info"] = *info;
    result["failure"] = nullptr;
    if (!config.errors.empty()) {
        const auto &error = config.errors.front();
        const char *reason = error.message == STR_NEWGRF_ERROR_READ_BOUNDS ? "ReadBounds" :
            error.message == STR_NEWGRF_ERROR_UNEXPECTED_SPRITE ? "UnexpectedSprite" : "OtherNativeError";
        result["failure"] = {{"reason", reason}, {"line", error.nfo_line}};
    }
    if (std::getenv("OTTD_GRF_SCAN_TRACE") != nullptr) {
        result["trace"] = RecordTrace(input);
        size_t size = 0;
        auto source = FioFOpenFile(input, "rb", NO_DIRECTORY, &size);
        if (!source.has_value()) throw std::runtime_error("missing trace file");
        result["trace"]["checksum_extent"] = std::min(size, GRFGetSizeOfDataSection(*source));
    }
    std::ofstream stream(output);
    stream << result.dump() << '\n';
    if (!stream) throw std::runtime_error("cannot write native GRF scan");
}
}
#endif
