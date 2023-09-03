use std::collections::HashMap;
use log::*;

pub use handle::{Bus, Handle};
pub use subcircuit::{SubcircuitBuilder, CircuitBuilder};

#[macro_use]
pub mod handle;
pub mod subcircuit;

pub struct Circuit {
    // construction
    num_nodes: usize,
    coils: Vec<Vec<Coil>>, // NodeId -> Coils
    switches: Vec<Switch>, // SwitchId -> Switch
    labels: HashMap<Handle, NodeId>,
    traces: HashMap<NodeId, bool>,
    sources: Vec<NodeId>,
    
    // state
    switch_positions: Vec<bool>, // SwitchId -> bool
    connections: Vec<Vec<NodeId>>, // NodeId -> NodeIds
    initialized: bool,
}

struct Coil {
    switches: Vec<SwitchId>,
}

struct Switch {
    pole: NodeId,
    no: NodeId,
    nc: NodeId,
}

pub type NodeId = usize;
type SwitchId = usize;

/// Simulates a collection of subcircuits
impl Circuit {

    pub fn set(&mut self, handle: &Handle) {
        self.sources.push(self.labels[handle]);
    }

    pub fn set_bus(&mut self, bus: &Bus, k: i32) {
        let mut max_index = 0i8;
        let mut min_index = 31i8;
        for (handle, node_id) in &self.labels {
            if handle.name == bus.name && handle.sup == bus.sup {
                let index = handle.index.expect("inspect_all called on a node with no index");
                if (k >> index) & 1 != 0 {
                    self.sources.push(*node_id);
                }
                max_index = std::cmp::max(max_index, index);
                min_index = std::cmp::min(min_index, index);
            }
        }
    }

    pub fn inspect(&self, handle: &Handle) -> bool {
        if let Some(node_id) = self.labels.get(handle) {
            info!("{}: {}", handle, if self.traces[node_id] { 1 } else { 0 });
            self.traces[node_id]
        } else {
            error!("Could not find node \"{}\" to inspect", handle);
            panic!();
        }
    }

    pub fn inspect_bus(&self, bus: &Bus) -> i32 {
        let mut ret = 0i32;
        let mut states = Vec::<(i8, bool)>::new();
        for (handle, node_id) in &self.labels {
            if handle.name == bus.name && handle.sup == bus.sup {
                states.push((handle.index.expect("inspect_all called on a node with no index"), self.traces[node_id]));
                if self.traces[node_id] {
                    ret |= 1 << handle.index.unwrap();
                }
            }
        }
        assert!(!states.is_empty());
        states.sort_by_key(|s| -s.0);
        let max_index = states[0].0;
        let min_index = states[states.len() - 1].0;
        ret = (ret << (32 - max_index - 1)) >> (32 - max_index - 1);
        info!("{}[{}:{}]: {}",
              bus,
              max_index,
              min_index,
              states.iter().map(|(_, state)| if *state { '1' } else { '0' }).collect::<String>());
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_circuit() {
        let mut c = CircuitBuilder::new()
            .add_subcircuit(|mut scb| {
                scb.label(handle!("Aa", 0));
                let shared = scb.label("shared");
                scb.trace(shared);
            })
            .add_subcircuit(|mut scb| {
                scb.label(handle!("Ba", 0));
                let shared = scb.label("shared");
                let g = scb.label("G");
                scb.add_switch("dummy", (shared, g, g));
            })
            .finalize();

        c.step();
        assert!(c.inspect(&handle!("shared")));
    }
}

