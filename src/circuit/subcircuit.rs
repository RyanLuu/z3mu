use std::collections::HashMap;
use std::iter::zip;
use log::*;

use super::{Coil, Switch, Handle, NodeId};

type SwitchId = usize;

/// Part of a circuit that may expose some nodes for other subcircuits to connect to
#[derive(Default)]
pub struct Subcircuit {
    // construction
    coils: Vec<Vec<Coil>>, // NodeId -> Coils
    switches: Vec<Switch>, // SwitchId -> Switch
    name_to_node: HashMap<Handle, NodeId>,
    pub public_nodes: HashMap<NodeId, Vec<Handle>>,
    
    // state
    switch_positions: Vec<bool>, // SwitchId -> bool
    next_switch_positions: Vec<bool>, // SwitchId -> bool
    connections: Vec<Vec<NodeId>>, // NodeId -> NodeIds
    node_states: Vec<bool>, // NodeId -> bool
}

/// A subcircuit in the process of being built
#[derive(Default)]
pub struct CBuilder {
    num_nodes: usize,
    node_aliases: HashMap<Handle, NodeId>,
    switches: Vec<BuilderSwitch>,
    coils: Vec<BuilderCoil>,
    public_nodes: HashMap<NodeId, Vec<Handle>>,
}

struct BuilderSwitch {
    name: Handle,
    pole: NodeId,
    no: NodeId,
    nc: NodeId,
}

struct BuilderCoil {
    name: Handle,
    pos: NodeId,
}

/// Specifies how to treat a node introduced to the circuit by a new component
///
/// * `Wire(node_id)` - Connect to the existing node identified by `node_id`
/// * `Named(name)` - Create a new node named `name` or connect to it if it already exists
/// * `New` - Create a new node in the circuit that is not yet connected to anything else
pub enum NodeSpec {
    Wire(NodeId),
    Named(Handle),
    New,
}

impl From<()> for NodeSpec {
    fn from(_: ()) -> NodeSpec {
        NodeSpec::New
    }
}

impl<T: Into<Handle>> From<T> for NodeSpec {
    fn from(handle: T) -> NodeSpec {
        NodeSpec::Named(handle.into())
    }
}

impl From<NodeId> for NodeSpec {
    fn from(id: NodeId) -> NodeSpec {
        NodeSpec::Wire(id)
    }
}

impl CBuilder {

    /// Create a new CBuilder with only the G node
    pub fn new() -> Self {
        CBuilder::default()
    }

    fn is_public_by_default(handle: &Handle) -> bool {
        const PUBLIC_LIST: &[&str] = &[
            "G", // positive voltage rail
            "S", // step nodes (e.g. "I II III")
            // registers of interest
            "Aa", // exponent summand
            "Ab", // exponent summand
            "Ae", // exponent sum
            "Ba", // mantissa summand
            "Bb", // mantissa summand
            "Be", // mantissa sum
        ];
        PUBLIC_LIST.contains(&handle.name.as_str())
    }

    pub fn coil_to_switch_name(coil_handle: &Handle) -> Handle {
        Handle::new(coil_handle.name.to_lowercase(), coil_handle.index, None)
    }

    /// Call after adding all components
    pub fn finalize(self) -> Subcircuit {
        let mut c = Subcircuit::default();

        // initialize nodes
        c.node_states = vec![false; self.num_nodes];
        for (node_id, names) in &self.public_nodes {
            for name in names {
                c.name_to_node.insert(name.clone(), *node_id);
            }
        }

        // initialize switches and connections
        c.connections = vec![Vec::new(); self.num_nodes];
        let mut switches_by_name: HashMap<Handle, Vec<SwitchId>> = HashMap::new();
        let num_switches = self.switches.len();
        c.switches.reserve(num_switches);
        for (id, switch) in self.switches.into_iter().enumerate() {
            Subcircuit::connect(&mut c.connections, switch.pole, switch.nc);
            if !switches_by_name.contains_key(&switch.name) {
                switches_by_name.insert(switch.name.clone(), Vec::new());
            }
            switches_by_name.get_mut(&switch.name).unwrap().push(id);
            c.switches.push(Switch::from(switch));
        }
        c.switch_positions = vec![false; num_switches];
        c.next_switch_positions = vec![false; num_switches];

        // initialize coils
        c.coils.reserve(self.num_nodes);
        for _ in 0..self.num_nodes{
            c.coils.push(Vec::new());
        }
        for coil in self.coils {
            let switches = switches_by_name
                .get(&CBuilder::coil_to_switch_name(&coil.name))
                .map_or_else(Vec::new, Vec::clone);

            if switches.is_empty() {
                warn!("Coil {} is not connected to any switches", coil.name);
            }

            c.coils[coil.pos].push(Coil { switches });
        }
        c.public_nodes = self.public_nodes;
        c
    }

    fn name_node(&mut self, n: NodeId, name: Handle) {
        let prev = self.node_aliases.insert(name.clone(), n);
        assert_eq!(prev, None);
        if CBuilder::is_public_by_default(&name) {
            self.expose_node(name);
        }
    }

    pub fn expose_node(&mut self, handle: Handle) {
        let node_id = self.node_aliases.get(&handle).expect(&format!("Failed to expose node {}", handle));
        self.public_nodes.entry(*node_id).or_insert_with(Vec::new).push(handle);
    }

    pub fn node(&mut self, spec: impl Into<NodeSpec>) -> NodeId {
        match spec.into() {
            NodeSpec::Wire(node) => node,
            NodeSpec::Named(name) => self.named_node(name),
            NodeSpec::New => self.add_node(),
        }
    }

    /// Adds a coil to the circuit and adds an alias for its positive terminal
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the coil, formatted according to Section 2.1
    /// * `pos` - NodeSpec of the coil's positive terminal
    ///
    /// # Examples
    ///
    /// ```
    /// let cb = CBuilder::new();
    /// let n0 = cb.node(NodeSpec::new);
    /// let n1 = cb.add_coil("Ba2", NodeSpec::New);
    /// let n2 = cb.add_coil("Ba2", NodeSpec::Wire(n0));
    /// assert_eq!(n0, n2);
    /// ```
    pub fn add_coil(&mut self, handle: impl Into<Handle>, pos: impl Into<NodeSpec>) -> NodeId {
        let pos = self.node(pos);
        let handle = handle.into();
        self.name_node(pos, handle.clone());
        self.coils.push(BuilderCoil { name: handle, pos });
        pos
    }

    pub fn add_switch(&mut self, name: impl Into<Handle>, loc: (impl Into<NodeSpec>, impl Into<NodeSpec>, impl Into<NodeSpec>)) -> (NodeId, NodeId, NodeId) {
        let pole = self.node(loc.0.into());
        let no = self.node(loc.1.into());
        let nc = self.node(loc.2.into());
        self.switches.push(BuilderSwitch { name: name.into(), pole, no, nc });
        (pole, no, nc)
    }

    fn add_node(&mut self) -> NodeId {
        let ret = self.num_nodes;
        self.num_nodes += 1;
        ret
    }

    fn named_node(&mut self, name: Handle) -> NodeId {
        if let Some(node_id) = self.node_aliases.get(&name) {
            *node_id
        } else {
            let ret = self.add_node();
            self.name_node(ret, name);
            ret
        }
    }

    pub fn chain<T, I, Idx, F>(init: T, iter: I, mut func: F) -> T where
        I: Iterator<Item = Idx>,
        F: FnMut(T, Idx) -> T
    {
        let mut curr = init;
        for idx in iter {
            curr = func(curr, idx);
        }
        curr
    }
}

impl Subcircuit {

    pub fn start_step(&mut self) {
        self.node_states = vec![false; self.node_states.len()];
    }

    /// Signal that a node has been pulled high and propagate effects through the circuit
    ///
    /// # Arguments
    ///
    /// * `node_name` - Alias for the node being pulled high
    pub fn update(&mut self, node_name: &Handle) -> Vec<Handle> {
        let mut worklist = vec![self.name_to_node[node_name]];
        let visited = &mut self.node_states;
        
        let mut ret = Vec::<Handle>::new();
        while let Some(node) = worklist.pop() {
            if visited[node] {
                continue;
            }
            if let Some(node_names) = self.public_nodes.get(&node) {
                for node_name in node_names {
                    ret.push(node_name.clone());
                }
            }
            visited[node] = true;

            for coil in &self.coils[node] {
                for switch in &coil.switches {
                    self.next_switch_positions[*switch] = true;
                }
            }
            for other in &self.connections[node] {
                if !visited[*other] {
                    worklist.push(*other);
                }
            }
        }
        ret
    }

    pub fn end_step(&mut self) {
        std::mem::swap(&mut self.switch_positions, &mut self.next_switch_positions);
        self.next_switch_positions = vec![false; self.switch_positions.len()];
        self.end_step_connections();
    }

    fn end_step_connections(&mut self) {
        let num_nodes = self.node_states.len();
        self.connections = vec![Vec::new(); num_nodes];
        for (active, switch) in zip(&self.switch_positions, &self.switches) {
            let branch = if *active {
                switch.no
            } else {
                switch.nc
            };
            Subcircuit::connect(&mut self.connections, switch.pole, branch);
        }
    }

    fn connect(connections: &mut Vec<Vec<NodeId>>, a: NodeId, b: NodeId) {
        connections[a].push(b);
        connections[b].push(a);
    }
}

impl From<BuilderSwitch> for Switch {
    fn from(bs: BuilderSwitch) -> Switch {
        Switch {
            pole: bs.pole,
            no: bs.no,
            nc: bs.nc,
        }
    }
}

#[cfg(test)]
mod tests {

    impl CBuilder {
        fn next_switch_id(&self) -> SwitchId {
            self.switches.len()
        }
    }

    impl Subcircuit {
        fn is_high(&self, node: &Handle) -> bool {
            self.node_states[self.name_to_node[node]]
        }

        fn is_low(&self, node: &Handle) -> bool {
            !self.node_states[self.name_to_node[node]]
        }

        fn is_active(&self, switch: SwitchId) -> bool {
            self.switch_positions[switch]
        }

        fn is_inactive(&self, switch: SwitchId) -> bool {
            !self.switch_positions[switch]
        }

        pub fn step(&mut self) {
            self.start_step();
            if self.name_to_node.contains_key(&handle!("G")) {
                self.update(&handle!("G"));
            }
            self.end_step();
        }
    }

    use super::*;
    use super::NodeSpec::*;

    #[test]
    fn empty() {
        let cb = CBuilder::new();
        let mut sc = cb.finalize();
        sc.step();
    }

    #[test]
    fn one_relay() {
        let mut cb = CBuilder::new();
        cb.add_coil("Ab0", "G");
        let switch_id = cb.next_switch_id();
        cb.add_switch("ab0", ("G", "no", "nc"));
        cb.expose_node(handle!("no"));
        cb.expose_node(handle!("nc"));
        let mut sc = cb.finalize();
        assert!(sc.is_inactive(switch_id));
        assert!(sc.is_low(&handle!("no")));
        assert!(sc.is_low(&handle!("nc")));
        sc.step(); // turn on
        assert!(sc.is_active(switch_id));
        assert!(sc.is_low(&handle!("no")));
        assert!(sc.is_high(&handle!("nc")));
        sc.step();
        assert!(sc.is_active(switch_id));
        assert!(sc.is_high(&handle!("no")));
        assert!(sc.is_low(&handle!("nc")));
    }

    #[test]
    fn oscillating_relay() {
        let mut cb = CBuilder::new();
        let switch_id = cb.next_switch_id();
        let (_, _, nc) = cb.add_switch("xy-10", ("G", New, "coil"));
        cb.add_coil("Xy-10", Wire(nc));
        cb.expose_node(handle!("coil"));
        let mut sc = cb.finalize();
        sc.step();
        for _ in 0..5 {
            assert!(sc.is_active(switch_id));
            assert!(sc.is_high(&handle!("coil")));
            sc.step();
            assert!(sc.is_inactive(switch_id));
            assert!(sc.is_low(&handle!("coil")));
            sc.step();
        }
    }

    #[test]
    fn step_subcircuit() {
        let mut cb = CBuilder::new();
        cb.add_coil("Init", "G");
        for i in 1..=5 {
            cb.add_coil(format!("S{}", i), format!("step{}", i));
            cb.expose_node(handle!(format!("step{}", i)));
        }
        cb.add_switch("init", ("G", New, "S1"));
        cb.add_switch("s1", ("G", "S2", New));
        cb.add_switch("s2", ("G", "S3", New));
        cb.add_switch("s3", ("G", "S4", New));
        cb.add_switch("s4", ("G", "S5", New));
        cb.add_switch("s5", ("G", "S1", New));

        cb.add_switch("init", ("G", New, "step123"));
        cb.add_switch("s5", ("G", "step123", New));
        cb.add_switch("s1", ("G", "step123", New));
        cb.add_switch("s2", ("G", "step123", New));
        cb.expose_node(handle!("step123"));
        let mut sc = cb.finalize();

        let test = |sc: &Subcircuit, expected_step: usize| {
            let mut expected_states = [false; 5];
            if expected_step != 0 {
                expected_states[expected_step - 1] = true;
            }
            for (i, expected) in expected_states.into_iter().enumerate() {
                assert_eq!(expected, sc.is_high(&handle!(format!("step{}", i + 1))));
            }
        };

        test(&sc, 0);
        for _ in 0..3 {
            for s in 1..=5 {
                sc.step();
                test(&sc, s);
                if s == 1 || s == 2 || s == 3 {
                    assert!(sc.is_high(&handle!("step123")));
                } else {
                    assert!(sc.is_low(&handle!("step123")));
                }
            }
        }
    }

    #[test]
    fn chain_alternating_relays() {
        let mut cb = CBuilder::new();
        let g = cb.node("G");
        let [last_a, last_b] = CBuilder::chain([g, g], 0..5, |[left_a, left_b], i| {
            cb.add_coil(format!("Bb{}", i), Wire(left_a));
            let right_a = cb.node(format!("a{}", i));
            let right_b = cb.node(format!("b{}", i));
            cb.expose_node(handle!(format!("a{}", i)));
            cb.expose_node(handle!(format!("b{}", i)));
            cb.add_switch(
                format!("aa{}", i),
                (Wire(left_a), Wire(right_a), New));
            cb.add_switch(
                format!("bb{}", i),
                (Wire(left_b), Wire(right_b), New));
            cb.add_coil(format!("Aa{}", i), Wire(right_b));
            [right_a, right_b]
        });
        assert_eq!(last_a, cb.node("a4"));
        assert_eq!(last_b, cb.node("b4"));
        let mut sc = cb.finalize();
        let test = |sc: &Subcircuit, expected_a: usize, expected_b: usize| {
            for i in 0..5 {
                assert_eq!(sc.is_high(&handle!(format!("a{}", i))), i < expected_a);
                assert_eq!(sc.is_high(&handle!(format!("b{}", i))), i < expected_b);
            }
        };

        sc.step();
        test(&sc, 0, 0);
        for i in 0..5 {
            sc.step();
            test(&sc, i, i + 1);
            sc.step();
            test(&sc, i + 1, i + 1);
        }
    }

}
