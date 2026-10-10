// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_LANGUAGE_TEXT_HPP
#define OTTD_REFERENCE_GRF_LANGUAGE_TEXT_HPP
namespace ReferenceGrfLanguageText {
nlohmann::json Snapshot()
{
    nlohmann::json entries = nlohmann::json::array();
    size_t index = 0;
    for (const auto &entry : _grf_text) {
        nlohmann::json texts = nlohmann::json::array();
        for (const auto &text : entry.textholder) {
            texts.push_back({{"language", text.langid}, {"bytes", std::vector<uint8_t>(text.text.begin(), text.text.end())}});
        }
        entries.push_back({{"index", index++}, {"grfid", entry.grfid}, {"local_id", entry.stringid.base()},
            {"default_id", entry.def_string}, {"texts", texts}});
    }
    return {{"selected", _current_lang_id}, {"entries", entries}};
}
}
#endif
