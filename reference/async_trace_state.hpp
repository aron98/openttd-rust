// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ASYNC_TRACE_STATE_HPP
#define OTTD_REFERENCE_ASYNC_TRACE_STATE_HPP
#include "../reference_async_trace.hpp"
#include "../linkgraph/linkgraphschedule.h"
#include "../network/network_internal.h"
#include "reference_job_annotations.hpp"
#include <map>

struct ReferenceAsyncAccess {
    static nlohmann::json Queues()
    {
        nlohmann::json scheduled = nlohmann::json::array(), running = nlohmann::json::array();
        for (const auto *graph : LinkGraphSchedule::instance.schedule) scheduled.push_back(graph->index.base());
        for (const auto *job : LinkGraphSchedule::instance.running) running.push_back(job->index.base());
        return {{"scheduled", scheduled}, {"running", running},
            {"job_pool", ReferenceOrderState::PoolState(_link_graph_job_pool)},
            {"graph_pool", ReferenceOrderState::PoolState(_link_graph_pool)}};
    }
};
namespace ReferenceAsync {
using Json = nlohmann::json;
struct Lifetime { uint64_t ordinal; uint32_t id; bool aborted = false; };
struct TraceState {
    std::ofstream output;
    std::map<uintptr_t, Lifetime> jobs;
    uint64_t sequence = 0, host = 0, next_lifetime = 0;
    TraceState()
    {
        output.exceptions(std::ios::failbit | std::ios::badbit);
        output.open(std::getenv("OTTD_ASYNC_TRACE"));
    }
};
TraceState &State()
{
    // Process-lifetime observer state survives native global pool cleanup.
    static TraceState *state = new TraceState();
    return *state;
}
void Write(const char *phase, Json fields = Json::object())
{
    auto &state = State();
    fields["event"] = phase;
    fields["sequence"] = state.sequence++;
    fields["host"] = state.host;
    fields["runtime"] = ReferenceReplay::Runtime();
    fields["network"] = {{"active", _networking}, {"server", _network_server},
        {"frame", _frame_counter}, {"frame_max", _frame_counter_max}, {"server_frame", _frame_counter_server}};
    state.output << fields.dump() << '\n';
    state.output.flush();
}
Json Identity(const LinkGraphJob *job)
{
    const auto &life = State().jobs.at(reinterpret_cast<uintptr_t>(job));
    return {{"job", life.id}, {"lifetime", life.ordinal}, {"aborted", life.aborted}};
}
void Event(const char *phase)
{
    if (Enabled()) Write(phase);
}
void QueueEvent(const char *phase)
{
    if (Enabled()) Write(phase, {{"queues", ReferenceAsyncAccess::Queues()}});
}
void Enter()
{
    ++State().host;
    Write("host_enter");
}
void Callback(const char *phase)
{
    if (!Enabled()) return;
    Json limits = Json::object();
    for (const Company *company : Company::Iterate()) limits[std::to_string(company->index.base())] = {
        {"terraform", company->terraform_limit}, {"clear", company->clear_limit},
        {"tree", company->tree_limit}, {"build_object", company->build_object_limit}};
    Write(phase, {{"limits", limits}});
}
void PauseRequest(bool pause)
{
    if (Enabled()) Write("pause_request", {{"mode", static_cast<uint32_t>(PauseMode::LinkGraph)}, {"pause", pause}});
}
void PauseApplied(uint32_t mode, uint32_t previous, bool pause)
{
    if (Enabled()) Write("pause_apply", {{"mode", mode}, {"previous", previous}, {"pause", pause}});
}
}
#endif
