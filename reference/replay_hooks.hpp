// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_REPLAY_HOOKS_HPP
#define OTTD_REFERENCE_REPLAY_HOOKS_HPP
namespace ReferenceReplay {
void Phase(const char *phase, const CommandCost &cost);
void Gate(const char *gate);
}
#endif
