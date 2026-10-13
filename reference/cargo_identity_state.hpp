// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CARGO_IDENTITY_STATE_HPP
#define OTTD_REFERENCE_CARGO_IDENTITY_STATE_HPP
#include "newgrf_cargo.h"
struct ReferenceCargoIdentityAccess {
    static const std::map<CargoLabel, CargoType> &LabelMap() { return CargoSpec::label_map; }
};
namespace ReferenceCargoIdentity {
using Json = nlohmann::json;

inline Json FileIdentity(const GRFFile *file)
{
    if (file == nullptr) return nullptr;
    for (size_t index = 0; index < _grf_files.size(); ++index) {
        if (&_grf_files[index] == file) {
            return {{"index", index}, {"grfid", _grf_files[index].grfid}};
        }
    }
    ReferenceGrfControl::HostError("cargo-identity host expired file reference");
}

inline Json Labels(std::span<const CargoLabel> labels)
{
    Json result = Json::array();
    for (CargoLabel label : labels) result.push_back(label.base());
    return result;
}

inline Json State()
{
    if (_grf_files.size() > 8) ReferenceGrfControl::HostError("cargo-identity host file capacity");
    Json owners = Json::array(), files = Json::array(), label_map = Json::array();
    for (CargoType id = 0; id < NUM_CARGO; ++id) {
        const CargoSpec &c = *CargoSpec::Get(id);
        if (c.group != nullptr) ReferenceGrfControl::HostError("cargo-identity host sprite-group scope");
        owners.push_back({{"id", id}, {"label", c.label.base()}, {"bitnum", c.bitnum},
            {"legend_colour", c.legend_colour.p}, {"rating_colour", c.rating_colour.p},
            {"weight", c.weight}, {"multiplier", c.multiplier}, {"classes", c.classes.base()},
            {"initial_payment", c.initial_payment}, {"transit_periods", c.transit_periods},
            {"is_freight", c.is_freight}, {"town_acceptance_effect", c.town_acceptance_effect},
            {"town_production_effect", c.town_production_effect},
            {"town_production_multiplier", c.town_production_multiplier},
            {"callback_mask", c.callback_mask.base()}, {"name", c.name}, {"name_single", c.name_single},
            {"units_volume", c.units_volume}, {"quantifier", c.quantifier}, {"abbrev", c.abbrev},
            {"sprite", c.sprite}, {"file", FileIdentity(c.grffile)}, {"group", nullptr},
            {"current_payment", static_cast<int64_t>(c.current_payment)},
            {"valid", c.IsValid()}, {"default", IsDefaultCargo(id)}});
    }
    for (const auto &[label, id] : ReferenceCargoIdentityAccess::LabelMap()) label_map.push_back({label.base(), id});
    for (size_t index = 0; index < _grf_files.size(); ++index) {
        const GRFFile &file = _grf_files[index];
        if (file.cargo_list.size() > 255) ReferenceGrfControl::HostError("cargo-identity host table capacity");
        files.push_back({{"index", index}, {"grfid", file.grfid}, {"version", file.grf_version},
            {"features", file.grf_features.base()}, {"parameters", file.param},
            {"cargo_list", Labels(file.cargo_list)}, {"fallback", file.cargo_list_is_fallback},
            {"cargo_map", file.cargo_map}, {"selected_table", Labels(GetCargoTranslationTable(file))}});
    }
    return {{"cargo", owners}, {"cargo_mask", _cargo_mask}, {"standard_cargo_mask", _standard_cargo_mask},
        {"label_map", label_map}, {"files", files},
        {"climate_dependent", Labels(GetClimateDependentCargoTranslationTable())},
        {"climate_independent", Labels(GetClimateIndependentCargoTranslationTable())},
        {"engines", ReferenceEngineSpecs::State(FileIdentity)}};
}

// Admission validates feature/property/stage/widths before constructing ByteReader.
// These are the actual upstream handlers, not implementations of their behavior.
inline ChangeInfoResult Invoke(GrfSpecFeature feature, bool reserve, uint first, uint last, int property, ByteReader &reader)
{
    switch (feature) {
        case GSF_CARGOES:
            return reserve ? GrfChangeInfoHandler<GSF_CARGOES>::Reserve(first, last, property, reader) :
                GrfChangeInfoHandler<GSF_CARGOES>::Activation(first, last, property, reader);
        case GSF_GLOBALVAR:
            return reserve ? GrfChangeInfoHandler<GSF_GLOBALVAR>::Reserve(first, last, property, reader) :
                GrfChangeInfoHandler<GSF_GLOBALVAR>::Activation(first, last, property, reader);
        case GSF_ROADVEHICLES:
            return reserve ? GrfChangeInfoHandler<GSF_ROADVEHICLES>::Reserve(first, last, property, reader) :
                GrfChangeInfoHandler<GSF_ROADVEHICLES>::Activation(first, last, property, reader);
        default: ReferenceGrfControl::HostError("cargo-identity host feature scope");
    }
}
}
#endif
