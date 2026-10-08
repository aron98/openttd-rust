// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_HOUSE_HPP
#define OTTD_REFERENCE_CALLBACK_HOUSE_HPP
#include "reference_callback_timer.hpp"
namespace ReferenceCallbackHouse {
using Json = nlohmann::json;
inline Json State()
{
    return {{"map", ReferenceGameplay::RawMap()}, {"random_state", {_random.state[0], _random.state[1]}}};
}
inline Json Run()
{
    Map::Allocate(64, 64);
    for (uint32_t i = 0; i < Map::Size(); ++i) {
        Tile t(i);
        t.type() = uint8_t((i % 11) << 4 | (i % 16));
        t.height() = uint8_t(i);
        t.m1() = uint8_t(i * 3); t.m2() = uint16_t(i * 7);
        t.m3() = uint8_t(i); t.m4() = uint8_t(i * 9); t.m5() = uint8_t(i);
        t.m6() = uint8_t(i * 13); t.m7() = uint8_t(i * 17); t.m8() = uint16_t(i * 19);
    }
    for (uint32_t age = 0; age < 256; ++age) {
        Tile complete(256 + age), incomplete(512 + age);
        complete.type() = 0x3f; complete.m3() |= 128; complete.m5() = uint8_t(age);
        incomplete.type() = 0x35; incomplete.m3() &= 127; incomplete.m5() = uint8_t(age);
    }
    Json cases = Json::array();
    for (const char *name : {"house_year_all_ages_raw_fields", "house_year_repeat"}) {
        Json before = State();
        ReferenceCallbackTimer::Invoke(TimerGameEconomy::YEAR, TimerGameEconomy::Priority::TOWN);
        cases.push_back({{"name", name}, {"before", before}, {"operation", {{"kind", "house_year"}}}, {"after", State()}});
    }
    return cases;
}
}
#endif
