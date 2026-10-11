// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ASYNC_TRACE_HPP
#define OTTD_REFERENCE_ASYNC_TRACE_HPP
#include <cstdint>
#include <cstdlib>
class LinkGraphJob;
namespace ReferenceAsync {
inline bool Enabled()
{
    static const bool enabled = std::getenv("OTTD_ASYNC_TRACE") != nullptr;
    return enabled;
}
void Event(const char *phase);
void QueueEvent(const char *phase);
void Sample(LinkGraphJob *job, bool due, int completed);
void Starting(LinkGraphJob *job);
void Backend(LinkGraphJob *job, bool threaded);
void JobEvent(LinkGraphJob *job, const char *phase);
void Joining(LinkGraphJob *job);
void Joined(LinkGraphJob *job, bool cleaning);
uint64_t Aborting(LinkGraphJob *job);
void AbortApplied(uint64_t lifetime);
void Freed(uint32_t id);
void PauseRequest(bool pause);
void PauseApplied(uint32_t mode, uint32_t previous, bool pause);
void Callback(const char *phase);
void Enter();
struct Scope {
    Scope() { if (Enabled()) Enter(); }
    ~Scope() { if (Enabled()) Event("host_exit"); }
};
}
#endif
