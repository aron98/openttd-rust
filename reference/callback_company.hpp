// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_COMPANY_HPP
#define OTTD_REFERENCE_CALLBACK_COMPANY_HPP
#include "reference_callback_timer.hpp"
namespace ReferenceCallbackCompany {
using Json = nlohmann::json;
inline Json State()
{
    Json state = {{"show_finances", _settings_client.gui.show_finances}, {"random_state", {_random.state[0], _random.state[1]}}, {"companies", Json::array()}};
    for (Company *c : Company::Iterate()) {
        Json rows = Json::array();
        for (const auto &row : c->yearly_expenses) {
            Json values = Json::array();
            for (Money expense : row) values.push_back(int64_t(expense));
            rows.push_back(values);
        }
        state["companies"].push_back({{"id", c->index.base()}, {"yearly_expenses", rows}});
    }
    return state;
}
inline Json Run()
{
    _company_pool.CleanPool();
    _settings_client.gui.show_finances = false;
    Json cases = Json::array();
    Json empty = State();
    ReferenceCallbackTimer::Invoke(TimerGameEconomy::YEAR, TimerGameEconomy::Priority::COMPANY);
    cases.push_back({{"name", "company_year_empty"}, {"before", empty}, {"operation", {{"kind", "company_year"}}}, {"after", State()}});
    for (uint8_t id : {0, 14}) {
        Company *c = new (CompanyID{id}) Company();
        uint n = 0;
        for (auto &row : c->yearly_expenses) {
            for (Money &expense : row) { expense = (++n % 2 == 0) ? INT64_MAX - n : INT64_MIN + n; }
        }
    }
    for (const char *name : {"company_year_signed_expenses", "company_year_repeat"}) {
        Json before = State();
        ReferenceCallbackTimer::Invoke(TimerGameEconomy::YEAR, TimerGameEconomy::Priority::COMPANY);
        cases.push_back({{"name", name}, {"before", before}, {"operation", {{"kind", "company_year"}}}, {"after", State()}});
    }
    return cases;
}
}
#endif
