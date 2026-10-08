// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_INDUSTRY_STATE_HPP
#define OTTD_REFERENCE_CALLBACK_INDUSTRY_STATE_HPP
#include "../industry.h"
#include "../company_func.h"
#include "../industrytype.h"
namespace ReferenceCallbackIndustry {
using Json = nlohmann::json;
inline Json State()
{
    Json state = {{"economy_type", int(_settings_game.economy.type)}, {"newgrf", false},
        {"industry_density", int(_settings_game.difficulty.industry_density)}, {"map_width", Map::SizeX()}, {"map_height", Map::SizeY()},
        {"current_company", _current_company.base()}, {"random_state", {_random.state[0], _random.state[1]}},
        {"wanted_inds", _industry_builder.wanted_inds}, {"industries", Json::array()}};
    for (Industry *i : Industry::Iterate()) {
        if (i->type >= NEW_INDUSTRYOFFSET || GetIndustrySpec(i->type)->callback_mask.Any() || GetIndustrySpec(i->type)->grf_prop.grfid != 0) throw std::runtime_error("Industry probe requires vanilla specs without callbacks");
        Json entry = {{"id", i->index.base()}, {"industry_type", i->type}, {"prod_level", i->prod_level},
            {"last_prod_year", i->last_prod_year.base()}, {"valid_history", i->valid_history}, {"produced", Json::array()}, {"accepted", Json::array()}};
        for (const auto &p : i->produced) {
            Json history = Json::array();
            for (const auto &h : p.history) history.push_back({{"production", h.production}, {"transported", h.transported}});
            entry["produced"].push_back({{"cargo", p.cargo}, {"waiting", p.waiting}, {"rate", p.rate}, {"history", history}});
        }
        for (const auto &a : i->accepted) {
            Json history = nullptr;
            if (a.history != nullptr) {
                history = Json::array();
                for (const auto &h : *a.history) history.push_back({{"accepted", h.accepted}, {"waiting", h.waiting}});
            }
            entry["accepted"].push_back({{"cargo", a.cargo}, {"waiting", a.waiting}, {"accumulated_waiting", a.accumulated_waiting},
                {"last_accepted", a.last_accepted.base()}, {"history", history}});
        }
        state["industries"].push_back(entry);
    }
    return state;
}
inline void Populate()
{
    _industry_pool.CleanPool();
    Industry::industries.fill({});
    for (auto [id, type] : {std::pair{0, 0}, std::pair{63999, 36}}) {
        Industry *i = new (IndustryID{uint16_t(id)}) Industry();
        i->type = uint8_t(type);
        Industry::industries[i->type].insert(i->index);
        i->prod_level = 16;
        i->last_prod_year = TimerGameEconomy::Year{1987};
        i->valid_history = UINT64_MAX;
        for (uint8_t cargo : {0, 1, 254, 255}) {
            auto &p = i->produced.emplace_back();
            p.cargo = cargo; p.waiting = uint16_t(cargo * 17); p.rate = cargo;
            for (uint n = 0; n < HISTORY_RECORDS; ++n) p.history[n] = {uint16_t(65535 - n * 53), uint16_t(n * 37)};
            auto &a = i->accepted.emplace_back();
            a.cargo = cargo; a.waiting = uint16_t(cargo * 19); a.accumulated_waiting = UINT32_MAX - cargo;
            a.last_accepted = TimerGameEconomy::Date{12345};
            if (cargo == 1) continue;
            auto &history = a.GetOrCreateHistory();
            for (uint n = 0; n < HISTORY_RECORDS; ++n) history[n] = {uint16_t(65535 - n * 43), uint16_t(n * 31)};
        }
    }
}
}
#endif
