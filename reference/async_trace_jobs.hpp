// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ASYNC_TRACE_JOBS_HPP
#define OTTD_REFERENCE_ASYNC_TRACE_JOBS_HPP
#include "reference_async_trace_state.hpp"
#include "reference_async_trace_input.hpp"
namespace ReferenceAsync {
void Starting(LinkGraphJob *job)
{
    if (!Enabled()) return;
    State().jobs.insert_or_assign(reinterpret_cast<uintptr_t>(job), Lifetime{State().next_lifetime++, job->index.base()});
    Json fields = Identity(job);
    fields["input"] = OwnedInput(job);
    fields["queues"] = ReferenceAsyncAccess::Queues();
    Write("spawn_begin", fields);
}
void Backend(LinkGraphJob *job, bool threaded)
{
    if (!Enabled()) return;
    Json fields = Identity(job);
    fields["threaded"] = threaded;
    Write("backend", fields);
}
void JobEvent(LinkGraphJob *job, const char *phase)
{
    if (!Enabled()) return;
    Json fields = Identity(job);
    if (!State().jobs.at(reinterpret_cast<uintptr_t>(job)).aborted) fields["queues"] = ReferenceAsyncAccess::Queues();
    Write(phase, fields);
}
void Sample(LinkGraphJob *job, bool due, int completed)
{
    if (!Enabled()) return;
    Json fields = job == nullptr ? Json{{"job", nullptr}, {"lifetime", nullptr}} : Identity(job);
    fields["due"] = job == nullptr ? Json(nullptr) : Json(due);
    fields["completed"] = completed < 0 ? Json(nullptr) : Json(completed != 0);
    fields["queues"] = ReferenceAsyncAccess::Queues();
    if (completed == 1 && !State().jobs.at(reinterpret_cast<uintptr_t>(job)).aborted) {
        fields["authorization"] = "branch_acquire";
        fields["annotations"] = ReferenceJobs::Completed(job);
    }
    Write("join_predicate", fields);
}
void Joining(LinkGraphJob *job)
{
    if (Enabled()) Write("destructor_join_begin", Identity(job));
}
void Joined(LinkGraphJob *job, bool cleaning)
{
    if (!Enabled()) return;
    Json fields = Identity(job);
    fields["cleaning"] = cleaning;
    if (!cleaning && !State().jobs.at(reinterpret_cast<uintptr_t>(job)).aborted) {
        fields["authorization"] = "actual_join_or_synchronous_return";
        fields["annotations"] = ReferenceJobs::Completed(job);
    }
    Write("joined", fields);
    if (cleaning) State().jobs.erase(reinterpret_cast<uintptr_t>(job));
}
uint64_t Aborting(LinkGraphJob *job)
{
    if (!Enabled()) return 0;
    // Bookkeeping before the native abort; no annotations are inspected here or later.
    State().jobs.at(reinterpret_cast<uintptr_t>(job)).aborted = true;
    Write("abort_begin", Identity(job));
    return State().jobs.at(reinterpret_cast<uintptr_t>(job)).ordinal;
}
void AbortApplied(uint64_t lifetime)
{
    if (!Enabled()) return;
    for (const auto &[token, life] : State().jobs) {
        if (life.ordinal == lifetime) {
            Write("abort_applied", {{"job", life.id}, {"lifetime", life.ordinal}, {"aborted", true}});
            return;
        }
    }
    throw std::runtime_error("missing observer abort lifetime");
}
void Freed(uint32_t id)
{
    if (!Enabled()) return;
    for (auto it = State().jobs.begin(); it != State().jobs.end(); ++it) {
        if (it->second.id == id) {
            Write("freed", {{"job", id}, {"lifetime", it->second.ordinal}, {"queues", ReferenceAsyncAccess::Queues()}});
            State().jobs.erase(it);
            return;
        }
    }
    throw std::runtime_error("missing observer freed lifetime");
}
}
#endif
