// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ASYNC_TRACE_INPUT_HPP
#define OTTD_REFERENCE_ASYNC_TRACE_INPUT_HPP
namespace ReferenceAsync {
Json OwnedInput(const LinkGraphJob *job)
{
    const auto &settings = job->Settings();
    Json nodes = Json::array();
    for (NodeID id = 0; id < job->Size(); ++id) {
        const auto &node = job->Graph()[id];
        Json edges = Json::array();
        for (const auto &edge : node.edges) edges.push_back({
            {"destination", edge.dest_node}, {"capacity", edge.capacity}, {"usage", edge.usage},
            {"travel_time_sum", edge.travel_time_sum}, {"last_unrestricted", edge.last_unrestricted_update.base()},
            {"last_restricted", edge.last_restricted_update.base()}});
        nodes.push_back({{"station", node.station.base()}, {"xy", node.xy.base()}, {"supply", node.supply},
            {"demand", node.demand}, {"last_update", node.last_update.base()}, {"edges", edges}});
    }
    return {{"graph", job->LinkGraphIndex().base()}, {"cargo", job->Cargo()},
        {"join_date", job->JoinDate().base()}, {"last_compression", job->LastCompression().base()},
        {"nodes", nodes}, {"settings", {
            {"recalc_time", settings.recalc_time}, {"recalc_interval", settings.recalc_interval},
            {"distribution_pax", settings.distribution_pax}, {"distribution_mail", settings.distribution_mail},
            {"distribution_armoured", settings.distribution_armoured}, {"distribution_default", settings.distribution_default},
            {"accuracy", settings.accuracy}, {"demand_size", settings.demand_size},
            {"demand_distance", settings.demand_distance}, {"short_path_saturation", settings.short_path_saturation}}}};
}
}
#endif
