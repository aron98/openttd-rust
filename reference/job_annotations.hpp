// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_JOB_ANNOTATIONS_HPP
#define OTTD_REFERENCE_JOB_ANNOTATIONS_HPP
#include "../linkgraph/linkgraphjob.h"
namespace ReferenceJobs {
using Json = nlohmann::json;
inline Json Completed(LinkGraphJob *job)
{
    Json nodes = Json::array();
    size_t edges = 0;
    for (NodeID id = 0; id < job->Size(); ++id) {
        const auto &node = (*job)[id];
        Json demands = Json::array(), edge_flows = Json::array(), paths = Json::array(), flows = Json::array();
        for (const auto &demand : node.demands) demands.push_back({demand.demand, demand.unsatisfied_demand});
        for (const auto &edge : node.edges) edge_flows.push_back({{"destination", edge.base.dest_node}, {"flow", edge.Flow()}});
        for (Path *path : node.paths) paths.push_back({{"node", path->GetNode()}, {"origin", path->GetOrigin()},
            {"parent", path->GetParent() == nullptr ? Json(nullptr) : Json(path->GetParent()->GetNode())},
            {"capacity", path->GetCapacity()}, {"free_capacity", path->GetFreeCapacity()}, {"distance", path->GetDistance()},
            {"flow", path->GetFlow()}, {"children", path->GetNumChildren()}});
        for (const auto &[origin, flow] : node.flows) {
            Json shares = Json::array();
            for (const auto &[amount, via] : *flow.GetShares()) shares.push_back({amount, via.base()});
            flows.push_back({{"origin", origin.base()}, {"unrestricted", flow.GetUnrestricted()}, {"shares", shares}});
        }
        nodes.push_back({{"undelivered_supply", node.undelivered_supply}, {"demands", demands},
            {"edge_flows", edge_flows}, {"paths", paths}, {"flows", flows}});
        edges += node.edges.size();
    }
    return {{"nodes", job->Size()}, {"edges", edges}, {"annotations", nodes}};
}
}
#endif
