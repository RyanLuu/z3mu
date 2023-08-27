use std::collections::{HashMap, HashSet};
use std::iter::zip;
use log::*;

#[derive(Default)]
pub struct Circuit {
    subcircuits: HashMap<SubcircuitId, Subcircuit>,
    node_rl: HashMap<String, Vec<SubcircuitId>>,
}

/// Simulates a collection of subcircuits
impl Circuit {
    pub fn new() -> Self {
        Circuit {
            subcircuits: HashMap::new(),
            node_rl: HashMap::new(),
        }
    }

    pub fn build_subcircuit<F>(&mut self, name: &str, public_nodes: Vec<&str>, build: F)
        where F: FnOnce(&mut CBuilder)
    {
        let mut cb = CBuilder::new();
        build(&mut cb);
        for node in public_nodes {
            cb.expose_node(node);
        }
        let sc = cb.finalize();
        self.add_subcircuit(name, sc);
    }

    pub fn add_subcircuit(&mut self, name: &str, subcircuit: Subcircuit) {
        for (_, node_names) in subcircuit.public_nodes.iter() {
            for node_name in node_names {
                if !self.node_rl.contains_key(node_name) {
                    self.node_rl.insert(node_name.clone(), Vec::new());
                }
                self.node_rl.get_mut(node_name).unwrap().push(String::from(name));
            }
        }
        self.subcircuits.insert(String::from(name), subcircuit);
    }

    pub fn get_subcircuit(&self, name: &str) -> &Subcircuit {
        self.subcircuits.get(name).unwrap()
    }

    pub fn step(&mut self) {
        let mut worklist = vec![String::from("G")];
        let mut visited = HashSet::<String>::new();
        while let Some(node_name) = worklist.pop() {
            if !visited.insert(node_name.clone()) {
                continue;
            }
            for scid in self.node_rl.get(&node_name).unwrap() {
                let subcircuit = self.subcircuits.get_mut(scid).unwrap();
                let nodes = subcircuit.update(&node_name);
                println!("{} {} {:?}", scid, node_name, nodes);
                worklist.extend(nodes);
            }
        }
    }
}

const G: NodeId = 0;

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
    name: String,
    pole: NodeId,
    no: NodeId,
    nc: NodeId,
}

type SubcircuitId = String;
type NodeId = usize;
type SwitchId = usize;

impl CBuilder {

    /// Create a new CBuilder with only the G node
    pub fn new() -> Self {
        let mut ret = CBuilder::default();
        ret.expose_node("G");
        ret
    }

    // TODO: change to From/Into
    fn finalize_switch(bs: BuilderSwitch) -> Switch {
        Switch {
            name: bs.name,
            pole: bs.pole,
            no: bs.no,
            nc: bs.nc,
        }
    }

    // TODO: change to From/Into
    fn finalize_coil(switches_by_name: &HashMap<String, Vec<SwitchId>>, bc: BuilderCoil) -> Coil {
        let maybe_switches = switches_by_name.get(&bc.name[..bc.name.find('^').unwrap_or(bc.name.len())].to_lowercase());
        if let Some(switches) = maybe_switches {
            Coil { switches: switches.clone() }
        } else {
            warn!("Coil {} is not connected to any switches", bc.name);
            Coil { switches: Vec::new() }
        }
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
        let mut switches_by_name: HashMap<String, Vec<SwitchId>> = HashMap::new();
        let num_switches = self.switches.len();
        c.switches.reserve(num_switches);
        for (id, switch) in self.switches.into_iter().enumerate() {
            Subcircuit::connect(&mut c.connections, switch.pole, switch.nc);
            if !switches_by_name.contains_key(&switch.name) {
                switches_by_name.insert(switch.name.clone(), Vec::new());
            }
            switches_by_name.get_mut(&switch.name).unwrap().push(id);
            c.switches.push(CBuilder::finalize_switch(switch));
        }
        c.switch_positions = vec![false; num_switches];
        c.next_switch_positions = vec![false; num_switches];

        // initialize coils
        c.coils.reserve(self.num_nodes);
        for _ in 0..self.num_nodes{
            c.coils.push(Vec::new());
        }
        for coil in self.coils {
            c.coils[coil.pos].push(CBuilder::finalize_coil(&switches_by_name, coil));
        }
        c.public_nodes = self.public_nodes;
        c
    }

    fn name_node(&mut self, n: NodeId, name: &str) {
        assert_eq!(None, self.node_aliases.insert(String::from(name), n));
    }

    pub fn expose_node(&mut self, name: &str) {
        let node_id = if !self.node_aliases.contains_key(name) {
            self.add_named_node(name)
        } else {
            self.node_aliases[name]
        };
        self.public_nodes.entry(node_id).or_insert_with(Vec::new).push(String::from(name));
    }

    pub fn get_node(&mut self, spec: NodeSpec) -> NodeId {
        match spec {
            NodeSpec::Wire(node) => node,
            NodeSpec::New => self.add_node(),
            NodeSpec::Named(name) => self.named_node(name),
        }
    }

    pub fn add_switch(&mut self, name: &str, loc: [NodeSpec; 3]) -> [NodeId; 3] {
        let [pole, no, nc] = loc.map(|spec| self.get_node(spec));
        self.switches.push(BuilderSwitch { name: String::from(name), pole, no, nc });
        [pole, no, nc]
    }

    pub fn add_node(&mut self) -> NodeId {
        let ret = self.num_nodes;
        self.num_nodes += 1;
        ret
    }

    fn named_node(&mut self, name: &str) -> NodeId {
        if let Some(node_id) = self.node_aliases.get(name) {
            *node_id
        } else {
            self.add_named_node(name)
        }
    }

    fn add_named_node(&mut self, name: &str) -> NodeId {
        let ret = self.add_node();
        self.name_node(ret, name);
        ret
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
    /// let n0 = cb.add_node();
    /// let n1 = cb.add_coil("Ba2", None);
    /// let n2 = cb.add_coil("Ba2", n0);
    /// assert_eq!(n0, n2);
    /// ```
    pub fn add_coil(&mut self, name: &str, pos: NodeSpec) -> NodeId {
        let pos = self.get_node(pos);
        self.name_node(pos, name);
        self.coils.push(BuilderCoil { name: String::from(name), pos });
        pos
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
        println!("LBHA {}", node_name);
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

    pub fn step(&mut self) {
        self.start_step();
        self.update("G");
        self.end_step();
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

    impl Subcircuit {
        fn is_high(&self, node: &str) -> bool {
            self.node_states[self.name_to_node[node]]
        }

        fn is_low(&self, node: &str) -> bool {
            !self.node_states[self.name_to_node[node]]
        }

        fn is_active(&self, switch: &str) -> bool {
            self.switch_positions[self.switch_name_to_id(switch)]
        }

        fn is_inactive(&self, switch: &str) -> bool {
            !self.switch_positions[self.switch_name_to_id(switch)]
        }

        fn switch_name_to_id(&self, name: &str) -> SwitchId {
            self.switches.iter().position(|s| s.name == name).expect(&format!("Did not find switch named \"{}\"", name))
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
        cb.add_coil("Ab0", Wire(G));
        cb.add_switch("ab0", [Wire(G), Named("no"), Named("nc")]);
        cb.expose_node("no");
        cb.expose_node("nc");
        let mut sc = cb.finalize();
        assert!(sc.is_inactive("ab0"));
        assert!(sc.is_low("no"));
        assert!(sc.is_low("nc"));
        sc.step(); // turn on
        assert!(sc.is_active("ab0"));
        assert!(sc.is_low("no"));
        assert!(sc.is_high("nc"));
        sc.step();
        assert!(sc.is_active("ab0"));
        assert!(sc.is_high("no"));
        assert!(sc.is_low("nc"));
    }

    #[test]
    fn oscillating_relay() {
        let mut cb = CBuilder::new();
        let [_, _, nc] = cb.add_switch("xy-10", [Wire(G), New, Named("coil")]);
        cb.add_coil("Xy-10", Wire(nc));
        cb.expose_node("coil");
        let mut sc = cb.finalize();
        sc.step();
        for _ in 0..5 {
            assert!(sc.is_active("xy-10"));
            assert!(sc.is_high("coil"));
            sc.step();
            assert!(sc.is_inactive("xy-10"));
            assert!(sc.is_low("coil"));
            sc.step();
        }
    }

    #[test]
    fn step_subcircuit() {
        let mut cb = CBuilder::new();
        cb.add_coil("Init", Wire(G));
        for i in 1..=5 {
            cb.add_coil(&format!("S{}", i), Named(&format!("step{}", i)));
            cb.expose_node(&format!("step{}", i));
        }
        cb.add_switch("init", [Wire(G), New, Named("S1")]);
        cb.add_switch("s1", [Wire(G), Named("S2"), New]);
        cb.add_switch("s2", [Wire(G), Named("S3"), New]);
        cb.add_switch("s3", [Wire(G), Named("S4"), New]);
        cb.add_switch("s4", [Wire(G), Named("S5"), New]);
        cb.add_switch("s5", [Wire(G), Named("S1"), New]);

        cb.add_switch("init", [Wire(G), New, Named("step123")]);
        cb.add_switch("s5", [Wire(G), Named("step123"), New]);
        cb.add_switch("s1", [Wire(G), Named("step123"), New]);
        cb.add_switch("s2", [Wire(G), Named("step123"), New]);
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
    fn basic_circuit() {
        let mut c = Circuit::new();
        c.build_subcircuit("A", vec!["shared"], |builder| {
            builder.add_named_node("Aa0");
            builder.expose_node("shared");
        });

        c.build_subcircuit("B", vec!["shared"], |builder| {
            builder.add_named_node("Ba0");
            builder.add_switch("dummy", [Named("shared"), Wire(G), Wire(G)]);
            builder.expose_node("shared");
        });

        c.step();

        assert!(c.get_subcircuit("A").is_high("shared"));
        assert!(c.get_subcircuit("B").is_high("shared"));
    }
}

