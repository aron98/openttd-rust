// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_HOOKS_HPP
#define OTTD_REFERENCE_REPLAY_HOOKS_HPP
namespace ReferenceReplay {
void Phase(const char *phase, const CommandCost &cost);
void Gate(const char *gate);
void LandscapeReturns(const char *phase, int64_t additional_money, uint32_t tile);
template <typename T>
void CapturePhase(const char *phase, const T &value)
{
    if constexpr (std::is_same_v<T, CommandCost>) {
        Phase(phase, value);
    } else {
        Phase(phase, std::get<0>(value));
        if constexpr (std::is_same_v<T, std::tuple<CommandCost, Money, TileIndex>>) {
            LandscapeReturns(phase, static_cast<int64_t>(std::get<1>(value)), std::get<2>(value).base());
        }
    }
}
}
#endif
