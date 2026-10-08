// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CALLBACK_TIMER_HPP
#define OTTD_REFERENCE_CALLBACK_TIMER_HPP
namespace ReferenceCallbackTimer {
inline void Invoke(TimerGameEconomy::Trigger trigger, TimerGameEconomy::Priority priority)
{
    auto &timers = TimerManager<TimerGameEconomy>::GetTimers();
    auto original = timers;
    for (auto it = timers.begin(); it != timers.end();) {
        if ((*it)->period.trigger == trigger && (*it)->period.priority == priority) ++it;
        else it = timers.erase(it);
    }
    if (timers.size() != 1) throw std::runtime_error("Original callback timer not found");
    _game_mode = GM_NORMAL;
    _settings_game.economy.timekeeping_units = TKU_CALENDAR;
    TimerGameEconomy::SetDate(TimerGameEconomy::ConvertYMDToDate(TimerGameEconomy::Year{2000}, 11, 31), 73);
    TimerManager<TimerGameEconomy>::Elapsed(1);
    timers = std::move(original);
}
}
#endif
