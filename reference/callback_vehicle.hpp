// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_VEHICLE_HPP
#define OTTD_REFERENCE_CALLBACK_VEHICLE_HPP
#include "../train.h"
#include "../roadveh.h"
#include "../ship.h"
#include "../aircraft.h"
#include "../company_base.h"
#include "../group.h"
#include "../vehicle_func.h"
#include "../core/pool_func.hpp"

namespace ReferenceCallbackVehicle {
using Json = nlohmann::json;
inline const char *Kind(VehicleType type)
{
    switch (type) {
        case VEH_TRAIN: return "train";
        case VEH_ROAD: return "road";
        case VEH_SHIP: return "ship";
        case VEH_AIRCRAFT: return "aircraft";
        default: throw std::runtime_error("Unsupported oracle vehicle type");
    }
}
inline Json State()
{
    Json result = {{"human_companies", Json::array()}, {"old_vehicle_warn", _settings_client.gui.old_vehicle_warn},
        {"vehicle_income_warn", _settings_client.gui.vehicle_income_warn}, {"random_state", {_random.state[0], _random.state[1]}},
        {"vehicles", Json::array()}, {"groups", Json::array()}};
    for (const Vehicle *v : Vehicle::Iterate()) {
        result["vehicles"].push_back({{"id", v->index.base()}, {"owner", v->owner.base()}, {"kind", Kind(v->type)},
            {"subtype", v->subtype}, {"group_id", v->group_id.base()}, {"age", v->age.base()}, {"max_age", v->max_age.base()},
            {"economy_age", v->economy_age.base()}, {"reliability_spd_dec", v->reliability_spd_dec},
            {"profit_this_year", int64_t(v->profit_this_year)}, {"profit_last_year", int64_t(v->profit_last_year)}});
    }
    auto group = [&](CompanyID owner, VehicleType type, GroupID id, const GroupStatistics &s) {
        result["groups"].push_back({{"owner", owner.base()}, {"kind", Kind(type)}, {"id", id.base()},
            {"profit_last_year", int64_t(s.profit_last_year)}, {"profit_last_year_min_age", int64_t(s.profit_last_year_min_age)},
            {"num_vehicle_min_age", s.num_vehicle_min_age}, {"num_vehicle", s.num_vehicle}});
    };
    for (Company *c : Company::Iterate()) {
        if (c->is_ai) throw std::runtime_error("Vehicle oracle requires human companies");
        result["human_companies"].push_back(c->index.base());
        for (VehicleType type = VEH_BEGIN; type < VEH_COMPANY_END; type++) {
            group(c->index, type, ALL_GROUP, c->group_all[type]);
            group(c->index, type, DEFAULT_GROUP, c->group_default[type]);
        }
    }
    for (Group *g : Group::Iterate()) group(g->owner, g->vehicle_type, g->index, g->statistics);
    return result;
}
inline Vehicle *Add(uint32_t id, VehicleType type, uint8_t subtype, CompanyID owner)
{
    Vehicle *v = nullptr;
    switch (type) {
        case VEH_TRAIN: v = new (VehicleID{id}) Train(); break;
        case VEH_ROAD: v = new (VehicleID{id}) RoadVehicle(); break;
        case VEH_SHIP: v = new (VehicleID{id}) Ship(); break;
        case VEH_AIRCRAFT: v = new (VehicleID{id}) Aircraft(); break;
        default: throw std::runtime_error("Unsupported oracle vehicle construction");
    }
    v->owner = owner;
    v->subtype = subtype;
    v->group_id = DEFAULT_GROUP;
    v->age = TimerGameCalendar::Date{999};
    v->max_age = TimerGameCalendar::Date{1000};
    v->economy_age = TimerGameEconomy::Date{731};
    v->reliability_spd_dec = 32769;
    v->profit_this_year = -1;
    v->profit_last_year = 987654;
    return v;
}
inline Json Run()
{
    _vehicle_pool.CleanPool();
    _group_pool.CleanPool();
    _company_pool.CleanPool();
    _game_mode = GM_NORMAL;
    _settings_client.gui.old_vehicle_warn = false;
    _settings_client.gui.vehicle_income_warn = false;
    _settings_game.economy.timekeeping_units = TKU_CALENDAR;
    _random.SetSeed(305419896U);
    Company *c0 = Company::GetIfValid(CompanyID{0});
    if (c0 == nullptr) c0 = new (CompanyID{0}) Company();
    Company *c1 = Company::GetIfValid(CompanyID{1});
    if (c1 == nullptr) c1 = new (CompanyID{1}) Company();
    Group *custom = new (GroupID{0}) Group(c0->index, VEH_TRAIN);
    std::vector<Vehicle *> vehicles;
    const std::pair<VehicleType, uint8_t> types[] = {{VEH_TRAIN, 9}, {VEH_TRAIN, 8}, {VEH_TRAIN, 4},
        {VEH_ROAD, 1}, {VEH_ROAD, 2}, {VEH_SHIP, 0}, {VEH_AIRCRAFT, 0}, {VEH_AIRCRAFT, 2}, {VEH_AIRCRAFT, 4}, {VEH_AIRCRAFT, 6}};
    uint32_t slot = 0;
    for (auto [type, subtype] : types) {
        Vehicle *v = Add(slot, type, subtype, c0->index);
        if (type == VEH_TRAIN) v->group_id = custom->index;
        vehicles.push_back(v);
        slot += 74;
    }
    vehicles.push_back(Add(1, VEH_SHIP, 0, c1->index));
    vehicles.push_back(Add(75, VEH_TRAIN, 9, c1->index));
    vehicles.back()->age = CalendarTime::MAX_DATE;
    Json cases = Json::array();
    auto calendar = [&](const char *name, uint16_t fraction) {
        Json before = State();
        TimerGameCalendar::date_fract = fraction;
        RunVehicleCalendarDayProc();
        cases.push_back({{"name", name}, {"before", before}, {"operation", {{"kind", "calendar_day"}, {"date_fract", fraction}}}, {"after", State()}});
    };
    calendar("calendar_subtypes_decay_wrap_sparse", 0);
    calendar("calendar_saturation_second_slot", 1);
    calendar("calendar_empty_slot", 73);
    for (int offset : {366, 731, 1096, 1461, 1462}) {
        for (Vehicle *v : vehicles) { v->age = TimerGameCalendar::Date{999 + offset}; v->reliability_spd_dec = 17; }
        std::string name = "calendar_anniversary_" + std::to_string(offset);
        calendar(name.c_str(), 0);
    }
    size_t i = 0;
    for (Vehicle *v : vehicles) {
        v->economy_age = TimerGameEconomy::Date{729 + int(i % 3)};
        v->profit_this_year = (i % 2 == 0) ? -257 - int(i) : 511 + int(i);
        ++i;
    }
    for (Company *c : Company::Iterate()) {
        for (VehicleType type = VEH_BEGIN; type < VEH_COMPANY_END; type++) {
            for (auto *s : {&c->group_all[type], &c->group_default[type]}) {
                s->num_vehicle = 27; s->num_vehicle_min_age = 19; s->profit_last_year = 123; s->profit_last_year_min_age = 456;
            }
        }
    }
    custom->statistics.num_vehicle = 31;
    auto &timers = TimerManager<TimerGameEconomy>::GetTimers();
    for (auto it = timers.begin(); it != timers.end();) {
        if ((*it)->period.trigger == TimerGameEconomy::YEAR && (*it)->period.priority == TimerGameEconomy::Priority::VEHICLE) ++it;
        else it = timers.erase(it);
    }
    if (timers.size() != 1) throw std::runtime_error("Original yearly vehicle callback not found");
    for (const char *name : {"economy_year_mixed_groups_thresholds", "economy_year_repeat_zero_profits"}) {
        Json before = State();
        TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year{2000}, 11, 31), 73);
        TimerManager<TimerGameEconomy>::Elapsed(1);
        cases.push_back({{"name", name}, {"before", before}, {"operation", {{"kind", "economy_year"}}}, {"after", State()}});
    }
    for (uint32_t id = 2000; id < 2300; ++id) Add(id, VEH_TRAIN, 9, c0->index);
    for (int64_t profit : {INT64_MAX, INT64_MIN}) {
        for (Vehicle *v : Vehicle::Iterate()) { v->profit_this_year = profit; v->economy_age = TimerGameEconomy::Date{731}; }
        Json before = State();
        TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year{2000}, 11, 31), 73);
        TimerManager<TimerGameEconomy>::Elapsed(1);
        cases.push_back({{"name", profit > 0 ? "economy_year_positive_money_saturation" : "economy_year_negative_money_saturation"},
            {"before", before}, {"operation", {{"kind", "economy_year"}}}, {"after", State()}});
    }
    return cases;
}
}
#endif
