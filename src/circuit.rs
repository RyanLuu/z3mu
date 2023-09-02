use std::collections::{HashMap, HashSet};
use log::*;

pub use handle::{Bus, Handle};
pub use subcircuit::{Subcircuit, CBuilder, NodeSpec};

#[macro_use]
pub mod handle;
pub mod subcircuit;

#[derive(Default)]
pub struct Circuit {
    subcircuits: HashMap<SubcircuitId, Subcircuit>,
    nodes: HashMap<Handle, PublicNode>,
    set_nodes: Vec<Handle>,
}

#[derive(Default, Debug)]
struct PublicNode {
    state: bool,
    subcircuits: Vec<SubcircuitId>,
}

struct Coil {
    switches: Vec<SwitchId>,
}

struct Switch {
    pole: NodeId,
    no: NodeId,
    nc: NodeId,
}

type SubcircuitId = String;
type NodeId = usize;
type SwitchId = usize;

pub struct Interface {
    nodes: Vec<Handle>,
}


/// Simulates a collection of subcircuits
impl Circuit {
    pub fn new() -> Self {
        Circuit::default()
    }

    pub fn set(&mut self, handle: &Handle) {
        if let Some(_) = self.nodes.get(handle) {
            self.set_nodes.push(handle.clone());
        } else {
            warn!("Could not find node \"{}\" to set", handle);
        }
    }

    pub fn set_bus(&mut self, bus: &Bus, k: i32) {
        let mut max_index = 0i8;
        let mut min_index = 31i8;
        for (handle, _) in &self.nodes {
            if handle.name == bus.name && handle.sup == bus.sup {
                let index = handle.index.expect("inspect_all called on a node with no index");
                if (k >> index) & 1 != 0 {
                    self.set_nodes.push(handle.clone());
                }
                max_index = std::cmp::max(max_index, index);
                min_index = std::cmp::min(min_index, index);
            }
        }
    }

    pub fn inspect(&self, handle: &Handle) -> bool {
        if let Some(public_node) = self.nodes.get(handle) {
            info!("{}: {}", handle, if public_node.state { 1 } else { 0 });
            public_node.state
        } else {
            error!("Could not find node \"{}\" to inspect", handle);
            panic!();
        }
    }

    pub fn inspect_bus(&self, bus: &Bus) -> i32 {
        let mut ret = 0i32;
        let mut states = Vec::<(i8, bool)>::new();
        for (handle, node) in &self.nodes {
            if handle.name == bus.name && handle.sup == bus.sup {
                states.push((handle.index.expect("inspect_all called on a node with no index"), node.state));
                if node.state {
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

    pub fn build_subcircuit<F>(&mut self, name: &str, build: F) where
        F: FnOnce(&mut CBuilder) -> Interface
    {
        let mut cb = CBuilder::new();
        let interface = build(&mut cb);
        for node in interface.nodes {
            cb.expose_node(node);
        }
        let sc = cb.finalize();
        self.add_subcircuit(name, sc);
    }

    fn add_subcircuit(&mut self, name: &str, subcircuit: Subcircuit) {
        for (_, node_names) in subcircuit.public_nodes.iter() {
            for node_name in node_names {
                if !self.nodes.contains_key(node_name) {
                    self.nodes.insert(node_name.clone(), PublicNode::default());
                }
                self.nodes.get_mut(node_name).unwrap().subcircuits.push(String::from(name));
            }
        }
        self.subcircuits.insert(String::from(name), subcircuit);
    }

    pub fn get_subcircuit(&self, name: &str) -> &Subcircuit {
        self.subcircuits.get(name).unwrap()
    }

    pub fn step(&mut self) {

        // reset public node states
        for (_, public_node) in &mut self.nodes {
            public_node.state = false;
        }

        for (_, sc) in &mut self.subcircuits {
            sc.start_step();
        }

        self.set_nodes.push(handle!("G"));
        let mut visited = HashSet::<Handle>::new();
        while let Some(node_name) = self.set_nodes.pop() {
            if !visited.insert(node_name.clone()) {
                continue;
            }
            if let Some(public_node) = self.nodes.get_mut(&node_name) {
                public_node.state = true;
                for scid in &public_node.subcircuits {
                    let subcircuit = self.subcircuits.get_mut(scid).unwrap();
                    let nodes = subcircuit.update(&node_name);
                    self.set_nodes.extend(nodes);
                }
            } else {
                warn!("Node {} is not connected to any subcircuits", node_name);
            }
        }

        for (_, sc) in &mut self.subcircuits {
            sc.end_step();
        }
    }
}

impl Interface {
    pub fn new<const N: usize>(nodes: [impl Into<Handle>; N]) -> Self {
        Interface {
            nodes: nodes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn empty() -> Self {
        Interface {
            nodes: Vec::new()
        }
    }

    pub fn push(&mut self, node: impl Into<Handle>) {
        self.nodes.push(node.into());
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_circuit() {
        let mut c = Circuit::new();
        c.build_subcircuit("A", |builder| {
            builder.node("Aa0");
            builder.node("shared");
            Interface::new(["shared"])
        });

        c.build_subcircuit("B", |builder| {
            builder.node("Ba0");
            builder.add_switch("dummy", ("shared", "G", "G"));
            Interface::new(["shared"])
        });

        c.step();
        assert!(c.inspect(&handle!("shared")));
    }
}

