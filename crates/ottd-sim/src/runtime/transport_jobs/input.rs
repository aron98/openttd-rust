//! Parse the saved graph/settings copy before allowing any worker to start.
use super::model::{Distribution, Edge, JobInput, Node, Settings};
use crate::{
    content::ContentCatalog,
    runtime::{RuntimeError, stations::saved::StationRow},
    world_access::row_field,
};
use ottd_save::WireValue;

pub(in crate::runtime) struct JobEnvironment<'a> {
    pub width: u32,
    pub height: u32,
    pub content: &'a ContentCatalog,
}
fn number<T: TryFrom<u64>>(row: StationRow<'_>, name: &str) -> Result<T, RuntimeError> {
    T::try_from(row.number(name)?).map_err(|_| RuntimeError::Invalid("job field width"))
}
fn date(row: StationRow<'_>, name: &str) -> Result<i32, RuntimeError> {
    match row_field(row.schema, row.record, name).map_err(|_| RuntimeError::Invalid("job date"))? {
        WireValue::Signed(value) => {
            i32::try_from(*value).map_err(|_| RuntimeError::Invalid("job date width"))
        }
        _ => Err(RuntimeError::Invalid("job signed date")),
    }
}
impl JobInput {
    pub(in crate::runtime) fn parse(
        job: StationRow<'_>,
        env: &JobEnvironment<'_>,
    ) -> Result<Option<Self>, RuntimeError> {
        let graph = job.child("linkgraph")?;
        let cargo = number::<u8>(graph, "cargo")?;
        let classes = env
            .content
            .cargo()
            .get(usize::from(cargo))
            .ok_or(RuntimeError::Invalid("job cargo"))?
            .classes;
        let distribution = number::<u8>(
            job,
            if classes & 1 != 0 {
                "linkgraph.distribution_pax"
            } else if classes & 2 != 0 {
                "linkgraph.distribution_mail"
            } else if classes & 8 != 0 {
                "linkgraph.distribution_armoured"
            } else {
                "linkgraph.distribution_default"
            },
        )?;
        let distribution = match distribution {
            0 => Distribution::Manual,
            1 => Distribution::Asymmetric,
            2 => Distribution::Symmetric,
            _ => return Ok(None),
        };
        let Some(nodes) = parse_nodes(graph)? else {
            return Ok(None);
        };
        let zero = nodes
            .iter()
            .all(|n| n.supply == 0 && n.demand == 0 && n.edges.iter().all(|e| e.usage == 0));
        if distribution != Distribution::Manual && !zero {
            return Ok(None);
        }
        Ok(Some(Self {
            graph: number(job, "link_graph.index")?,
            cargo,
            width: env.width,
            height: env.height,
            express: classes & 7 != 0,
            settings: Settings {
                distribution,
                accuracy: number(job, "linkgraph.accuracy")?,
                demand_distance: number(job, "linkgraph.demand_distance")?,
                demand_size: number(job, "linkgraph.demand_size")?,
                saturation: number(job, "linkgraph.short_path_saturation")?,
                recalc_time: number(job, "linkgraph.recalc_time")?,
            },
            join_date: date(job, "join_date")?,
            last_compression: date(graph, "last_compression")?,
            nodes,
        }))
    }
}
fn parse_nodes(graph: StationRow<'_>) -> Result<Option<Vec<Node>>, RuntimeError> {
    let (schema, rows) = graph.rows("nodes")?;
    let size = u16::try_from(rows.len()).map_err(|_| RuntimeError::Invalid("job node count"))?;
    let mut nodes = Vec::with_capacity(rows.len());
    for record in rows {
        let node = StationRow { schema, record };
        let (schema, records) = node.rows("edges")?;
        let mut edges = Vec::with_capacity(records.len());
        for record in records {
            let edge = StationRow { schema, record };
            let capacity = number::<u32>(edge, "capacity")?;
            let destination = number::<u16>(edge, "dest_node")?;
            if capacity == 0 || destination >= size {
                return Ok(None);
            }
            let divisor = std::num::NonZeroU64::new(u64::from(capacity))
                .ok_or(RuntimeError::Invalid("job edge capacity"))?;
            let time = edge.number("travel_time_sum")? / divisor;
            edges.push(Edge {
                destination,
                capacity,
                usage: number(edge, "usage")?,
                travel_time: u32::try_from(time & u64::from(u32::MAX))
                    .map_err(|_| RuntimeError::Invalid("job travel time"))?,
            });
        }
        nodes.push(Node {
            station: number(node, "station")?,
            xy: number(node, "xy")?,
            supply: number(node, "supply")?,
            demand: number(node, "demand")?,
            edges,
        });
    }
    Ok(Some(nodes))
}

pub(in crate::runtime) fn graph_supported(graph: StationRow<'_>) -> Result<bool, RuntimeError> {
    Ok(parse_nodes(graph)?.is_some())
}
