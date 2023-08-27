use std::collections::{HashMap, HashSet};
use std::iter::zip;
use log::*;

#[derive(Default)]
pub struct Circuit {
    subcircuits: HashMap<SubcircuitId, Subcircuit>,
    nodes: HashMap<String, PublicNode>,
}

#[derive(Default, Debug)]
struct PublicNode {
    state: bool,
    subcircuits: Vec<SubcircuitId>,
}

/// Simulates a collection of subcircuits
impl Circuit {
    pub fn new() -> Self {
        Circuit::default()
    }

    pub fn inspect(&self, name: &str) {
       if let Some(public_node) = self.nodes.get(name) {
           info!("{}: {}", name, if public_node.state { 1 } else { 0 });
       } else {
           warn!("Could not find node \"{}\" to inspect", name);
       }
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

        let mut worklist = vec![String::from("G")];
        let mut visited = HashSet::<String>::new();
        while let Some(node_name) = worklist.pop() {
            if !visited.insert(node_name.clone()) {
                continue;
            }
            debug!("{} {:#?}", node_name, self.nodes);
            if let Some(public_node) = self.nodes.get_mut(&node_name) {
                public_node.state = true;
                for scid in &public_node.subcircuits {
                    let subcircuit = self.subcircuits.get_mut(scid).unwrap();
                    let nodes = subcircuit.update(&node_name);
                    worklist.extend(nodes);
                }
            } else {
                warn!("Node {} is not connected to any subcircuits", node_name);
            }
        }
    }
}

/// A subcircuit in the process of being built
#[derive(Default)]
pub struct CBuilder {
    num_nodes: usize,
    node_aliases: HashMap<String, NodeId>,
    switches: Vec<BuilderSwitch>,
    coils: Vec<BuilderCoil>,
    public_nodes: HashMap<NodeId, Vec<String>>,
}

struct BuilderSwitch {
    name: String,
    pole: NodeId,
    no: NodeId,
    nc: NodeId,
}

struct BuilderCoil {
    name: String,
    pos: NodeId,
}

/// Specifies how to treat a node introduced to the circuit by a new component
///
/// * `Wire(node_id)` - Connect to the existing node identified by `node_id`
/// * `New` - Create a new node in the circuit that is connected to nothing else
/// * `Named(name)` - Create a new node named `name` or connect to it if it already exists
pub enum NodeSpec<'a> {
    Wire(NodeId),
    New,
    Named(&'a str),
}

/// Part of a circuit that may expose some nodes for other subcircuits to connect to
#[derive(Default)]
pub struct Subcircuit {
    // construction
    coils: Vec<Vec<Coil>>, // NodeId -> Coils
    switches: Vec<Switch>, // SwitchId -> Switch
    name_to_node: HashMap<String, NodeId>,
    public_nodes: HashMap<NodeId, Vec<String>>,
    
    // state
    switch_positions: Vec<bool>, // SwitchId -> bool
    next_switch_positions: Vec<bool>, // SwitchId -> bool
    connections: Vec<Vec<NodeId>>, // NodeId -> NodeIds
    node_states: Vec<bool>, // NodeId -> bool
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

impl From<BuilderSwitch> for Switch {
    fn from(bs: BuilderSwitch) -> Switch {
        Switch {
            pole: bs.pole,
            no: bs.no,
            nc: bs.nc,
        }
    }
}

impl CBuilder {

    /// Create a new CBuilder with only the G node
    pub fn new() -> Self {
        CBuilder::default()
    }

    fn is_public_by_default(node_name: &str) -> bool {
        node_name.chars().all(|c| c.is_ascii_uppercase() || c == '_')
    }

    pub fn coil_to_switch_name(coil_name: &str) -> String {
        coil_name[..coil_name.find('^').unwrap_or(coil_name.len())].to_lowercase()
    }

    /// Call after adding all components
    fn finalize(self) -> Subcircuit {
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
        let mut switches_by_name: HashMap<String, Vec<SwitchId>> = HashMap::new();
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

    fn name_node(&mut self, n: NodeId, name: String) {
        let prev = self.node_aliases.insert(name.clone(), n);
        assert_eq!(prev, None);
        if CBuilder::is_public_by_default(&name) {
            self.expose_node(name);
        }
    }

    fn expose_node<T: Into<String>>(&mut self, name: T) {
        let name_string = name.into();
        let node_id = self.node_aliases[&name_string];
        self.public_nodes.entry(node_id).or_insert_with(Vec::new).push(name_string);
    }

    pub fn node(&mut self, spec: NodeSpec) -> NodeId {
        match spec {
            NodeSpec::Wire(node) => node,
            NodeSpec::New => self.add_node(),
            NodeSpec::Named(name) => self.named_node(name),
        }
    }

    pub fn add_switch<T: Into<String>>(&mut self, name: T, loc: [NodeSpec; 3]) -> [NodeId; 3] {
        let [pole, no, nc] = loc.map(|spec| self.node(spec));
        self.switches.push(BuilderSwitch { name: name.into(), pole, no, nc });
        [pole, no, nc]
    }

    fn add_node(&mut self) -> NodeId {
        let ret = self.num_nodes;
        self.num_nodes += 1;
        ret
    }

    fn named_node(&mut self, name: &str) -> NodeId {
        if let Some(node_id) = self.node_aliases.get(name) {
            *node_id
        } else {
            let ret = self.add_node();
            self.name_node(ret, name.to_string());
            ret
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
    pub fn add_coil<T: Into<String>>(&mut self, name: T, pos: NodeSpec) -> NodeId {
        let pos = self.node(pos);
        let name_string = name.into();
        self.name_node(pos, name_string.clone());
        self.coils.push(BuilderCoil { name: name_string, pos });
        pos
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
    pub fn update(&mut self, node_name: &str) -> Vec<String> {
        let mut worklist = vec![self.name_to_node[node_name]];
        let visited = &mut self.node_states;
        
        let mut ret = Vec::<String>::new();
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

#[cfg(test)]
mod tests {

    impl CBuilder {
        fn next_switch_id(&self) -> SwitchId {
            self.switches.len()
        }
    }

    impl Subcircuit {
        fn is_high(&self, node: &str) -> bool {
            self.node_states[self.name_to_node[node]]
        }

        fn is_low(&self, node: &str) -> bool {
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
            if self.name_to_node.contains_key("G") {
                self.update("G");
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
        cb.add_coil("Ab0", Named("G"));
        let switch_id = cb.next_switch_id();
        cb.add_switch("ab0", [Named("G"), Named("no"), Named("nc")]);
        cb.expose_node("no");
        cb.expose_node("nc");
        let mut sc = cb.finalize();
        assert!(sc.is_inactive(switch_id));
        assert!(sc.is_low("no"));
        assert!(sc.is_low("nc"));
        sc.step(); // turn on
        assert!(sc.is_active(switch_id));
        assert!(sc.is_low("no"));
        assert!(sc.is_high("nc"));
        sc.step();
        assert!(sc.is_active(switch_id));
        assert!(sc.is_high("no"));
        assert!(sc.is_low("nc"));
    }

    #[test]
    fn oscillating_relay() {
        let mut cb = CBuilder::new();
        let switch_id = cb.next_switch_id();
        let [_, _, nc] = cb.add_switch("xy-10", [Named("G"), New, Named("coil")]);
        cb.add_coil("Xy-10", Wire(nc));
        cb.expose_node("coil");
        let mut sc = cb.finalize();
        sc.step();
        for _ in 0..5 {
            assert!(sc.is_active(switch_id));
            assert!(sc.is_high("coil"));
            sc.step();
            assert!(sc.is_inactive(switch_id));
            assert!(sc.is_low("coil"));
            sc.step();
        }
    }

    #[test]
    fn step_subcircuit() {
        let mut cb = CBuilder::new();
        cb.add_coil("Init", Named("G"));
        for i in 1..=5 {
            cb.add_coil(format!("S{}", i), Named(&format!("step{}", i)));
            cb.expose_node(format!("step{}", i));
        }
        cb.add_switch("init", [Named("G"), New, Named("S1")]);
        cb.add_switch("s1", [Named("G"), Named("S2"), New]);
        cb.add_switch("s2", [Named("G"), Named("S3"), New]);
        cb.add_switch("s3", [Named("G"), Named("S4"), New]);
        cb.add_switch("s4", [Named("G"), Named("S5"), New]);
        cb.add_switch("s5", [Named("G"), Named("S1"), New]);

        cb.add_switch("init", [Named("G"), New, Named("step123")]);
        cb.add_switch("s5", [Named("G"), Named("step123"), New]);
        cb.add_switch("s1", [Named("G"), Named("step123"), New]);
        cb.add_switch("s2", [Named("G"), Named("step123"), New]);
        cb.expose_node("step123");
        let mut sc = cb.finalize();

        let test = |sc: &Subcircuit, expected_step: usize| {
            let mut expected_states = [false; 5];
            if expected_step != 0 {
                expected_states[expected_step - 1] = true;
            }
            for (i, expected) in expected_states.into_iter().enumerate() {
                assert_eq!(expected, sc.is_high(&format!("step{}", i + 1)));
            }
        };

        test(&sc, 0);
        for _ in 0..3 {
            for s in 1..=5 {
                sc.step();
                test(&sc, s);
                if s == 1 || s == 2 || s == 3 {
                    assert!(sc.is_high("step123"));
                } else {
                    assert!(sc.is_low("step123"));
                }
            }
        }
    }

    #[test]
    fn chain_alternating_relays() {
        let mut cb = CBuilder::new();
        let g = cb.node(Named("G"));
        let [last_a, last_b] = CBuilder::chain([g, g], 0..5, |[left_a, left_b], i| {
            cb.add_coil(format!("Bb{}", i), Wire(left_a));
            let right_a = cb.node(Named(&format!("a{}", i)));
            let right_b = cb.node(Named(&format!("b{}", i)));
            cb.expose_node(format!("a{}", i));
            cb.expose_node(format!("b{}", i));
            cb.add_switch(
                format!("aa{}", i),
                [Wire(left_a), Wire(right_a), New]);
            cb.add_switch(
                format!("bb{}", i),
                [Wire(left_b), Wire(right_b), New]);
            cb.add_coil(format!("Aa{}", i), Wire(right_b));
            [right_a, right_b]
        });
        assert_eq!(last_a, cb.node(Named("a4")));
        assert_eq!(last_b, cb.node(Named("b4")));
        let mut sc = cb.finalize();
        let test = |sc: &Subcircuit, expected_a: usize, expected_b: usize| {
            for i in 0..5 {
                assert_eq!(sc.is_high(&format!("a{}", i)), i < expected_a);
                assert_eq!(sc.is_high(&format!("b{}", i)), i < expected_b);
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

    #[test]
    fn basic_circuit() {
        let mut c = Circuit::new();
        c.build_subcircuit("A", |builder| {
            builder.node(Named("Aa0"));
            builder.node(Named("shared"));
            Interface::new(&["shared"])
        });

        c.build_subcircuit("B", |builder| {
            builder.node(Named("Ba0"));
            builder.add_switch("dummy", [Named("shared"), Named("G"), Named("G")]);
            Interface::new(&["shared"])
        });

        c.step();

        assert!(c.get_subcircuit("A").is_high("shared"));
        assert!(c.get_subcircuit("B").is_high("shared"));
    }
}

pub struct Interface {
    nodes: Vec<String>,
}

impl Interface {
    pub fn new(nodes: &[&str]) -> Self {
        Interface {
            nodes: nodes.iter().map(ToString::to_string).collect(),
        }
    }

    pub fn push<T: Into<String>>(&mut self, node: T) {
        self.nodes.push(node.into());
    }
}

