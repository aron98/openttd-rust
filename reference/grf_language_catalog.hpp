// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_GRF_LANGUAGE_CATALOG_HPP
#define OTTD_REFERENCE_GRF_LANGUAGE_CATALOG_HPP
namespace ReferenceGrfLanguageCatalog {
using Json = nlohmann::json;
static LanguageList retained_prior_catalog;
static bool used = false;
[[noreturn]] static void HostError(std::string_view message)
{
    fmt::print(stderr, "GRF language catalog host refusal: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
static Json Header(const LanguagePackHeader &pack)
{
    Json genders = Json::array(), cases = Json::array(), tables = Json::array();
    for (const auto &slot : pack.genders) genders.push_back(std::vector<uint8_t>(std::begin(slot), std::end(slot)));
    for (const auto &slot : pack.cases) cases.push_back(std::vector<uint8_t>(std::begin(slot), std::end(slot)));
    for (auto count : pack.offsets) tables.push_back(FROM_LE16(count));
    return {{"language", pack.newgrflangid}, {"gender_count", pack.num_genders}, {"case_count", pack.num_cases},
        {"plural", pack.plural_form}, {"genders", genders}, {"cases", cases}, {"tables", tables}};
}
static Json Catalog()
{
    Json result = Json::array();
    for (const auto &pack : _languages) result.push_back({{"path", FS2OTTD(pack.file.native())}, {"header", Header(pack)}});
    return result;
}
Json Prepare(const Json &input)
{
    if (used) HostError("catalog fixture used twice");
    used = true;
    Json before = Catalog();
    Json prior_selection = _current_language == nullptr ? Json(nullptr) : Header(*_current_language);
    _current_language = nullptr;
    retained_prior_catalog = std::move(_languages);
    _languages.clear();
    Json admissions = Json::array(), sources = Json::array();
    const auto &directories = input.at("pack_directories");
    if (directories.size() > 256) HostError("too many pack directories");
    for (const auto &entry : directories) {
        std::string directory = entry.get<std::string>();
        std::vector<std::filesystem::path> files;
        for (const auto &file : std::filesystem::directory_iterator(OTTD2FS(directory))) {
            if (!file.is_regular_file() || file.path().extension() != ".lng") HostError("pack directory contains non-pack entry");
            files.push_back(file.path());
        }
        if (files.size() != 1) HostError("pack directory must contain one original input");
        std::string path = FS2OTTD(files.front().native());
        LanguagePackHeader header;
        bool valid = GetLanguageFileHeader(path, &header);
        bool duplicate = valid && GetLanguage(header.newgrflangid) != nullptr;
        size_t count = _languages.size();
        FillLanguageList(directory);
        if (_languages.size() != count + (valid && !duplicate ? 1 : 0)) HostError("original discovery result inconsistent");
        admissions.push_back(!valid ? "invalid" : duplicate ? "duplicate" : "accepted");
        sources.push_back(path);
    }
    uint8_t selected = input.at("selected").get<uint8_t>();
    const LanguageMetadata *pack = GetLanguage(selected);
    if (pack == nullptr) HostError("selected language unavailable");
    if (!ReadLanguagePack(pack)) HostError("original selected language body rejected");
    if (_current_language != pack) HostError("original selected pack identity changed");
    return {{"before", before}, {"prior_selection", prior_selection}, {"catalog", Catalog()},
        {"admissions", admissions}, {"sources", sources}, {"selected", selected},
        {"selected_header", Header(*_current_language)}, {"selected_strings", _langpack.strings.size()},
        {"pack_version", LANGUAGE_PACK_VERSION}, {"plural_rules", LANGUAGE_MAX_PLURAL}, {"plural_forms", LANGUAGE_MAX_PLURAL_FORMS}};
}
}
#endif
