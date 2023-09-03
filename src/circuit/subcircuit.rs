use std::collections::HashMap;
use std::iter::zip;
use log::*;

use super::{Circuit, Coil, Switch, Handle, NodeId};

type SwitchId = usize;

#[derive(Default)]
pub struct CircuitBuilder {
    num_nodes: usize,
    switches: Vec<BuilderSwitch>,
    coils: HashMap<Handle, NodeId>,
    labels: HashMap<Handle, NodeId>,
    traces: Vec<NodeId>,
}

/// A subcircuit in the process of being built
pub struct SubcircuitBuilder<'a> {
    cb: &'a mut CircuitBuilder,
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

impl CircuitBuilder {
    pub fn new() -> Self {
        let mut ret = CircuitBuilder::default();

        // boostrap with G node
        ret.labels.insert(handle!("G"), 0);
        ret.num_nodes += 1;

        ret
    }

    pub fn add_subcircuit<F: FnOnce(SubcircuitBuilder)>(mut self, build: F) -> Self {
        let scb = SubcircuitBuilder { cb: &mut self };
        build(scb);
        self
    }

    pub fn finalize(self) -> Circuit {
        // initialize switches and connections
        let mut switches_by_name: HashMap<Handle, Vec<SwitchId>> = HashMap::new();
        let mut switches = Vec::<Switch>::new();
        switches.reserve_exact(self.switches.len());
        for (id, switch) in self.switches.into_iter().enumerate() {
            if !switches_by_name.contains_key(&switch.name) {
                switches_by_name.insert(switch.name.clone(), Vec::new());
            }
            switches_by_name.get_mut(&switch.name).unwrap().push(id);
            switches.push(Switch::from(switch));
        }

        // initialize coils
        let mut coils = Vec::<Vec<Coil>>::new();
        coils.reserve(self.num_nodes);
        for _ in 0..self.num_nodes{
            coils.push(Vec::new());
        }
        for (coil_handle, coil_pos) in self.coils {
            let switches = switches_by_name
                .get(&CircuitBuilder::coil_to_switch_name(&coil_handle))
                .map_or_else(Vec::new, Vec::clone);

            if switches.is_empty() {
                warn!("Coil {} is not connected to any switches", coil_handle);
            }

            coils[coil_pos].push(Coil { switches });
        }
        let traces: HashMap<NodeId, bool> = self.traces.into_iter().map(|node_id| (node_id, false)).collect();

        let mut ret = Circuit {
            num_nodes: self.num_nodes,
            coils,
            switches,
            labels: self.labels,
            traces,
            sources: Vec::new(),

            switch_positions: Vec::new(),
            connections: Vec::new(),
            initialized: false,
        };
        ret.step(); // initialize connections and switch_positions
        ret.initialized = true;
        ret
    }

    pub fn coil_to_switch_name(coil_handle: &Handle) -> Handle {
        Handle::new(coil_handle.name.to_lowercase(), coil_handle.index, None)
    }
}

impl<'a> SubcircuitBuilder<'a> {

    fn new_node(&mut self) -> NodeId {
        let new_node = self.cb.num_nodes;
        self.cb.num_nodes += 1;
        return new_node;
    }

    pub fn label(&mut self, label: impl Into<Handle>) -> NodeId {
        let handle: Handle = label.into();
        if let Some(existing) = self.cb.labels.get(&handle) {
            *existing
        } else {
            let new_node = self.new_node();
            self.cb.labels.insert(handle, new_node);
            new_node
        }
    }

    pub fn trace(&mut self, node: NodeId) {
        self.cb.traces.push(node);
    }

    pub fn trace_all(&mut self, nodes: impl IntoIterator<Item = NodeId>) {
        for node in nodes.into_iter() {
            self.trace(node);
        }
    }

    pub fn node(&mut self, maybe_node: Option<NodeId>) -> NodeId {
        if let Some(existing) = maybe_node {
            existing
        } else {
            self.new_node()
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
    /// let cb = CircuitBuilder::new();
    /// let n0 = cb.node(NodeSpec::new);
    /// let n1 = cb.add_coil("Ba2", NodeSpec::None);
    /// let n2 = cb.add_coil("Ba2", NodeSpec::Some(n0));
    /// assert_eq!(n0, n2);
    /// ```
    pub fn add_coil(&mut self, handle: impl Into<Handle>, pos: impl Into<Option<NodeId>>) -> NodeId {
        let handle = handle.into();
        if self.cb.labels.contains_key(&handle) {
            assert_eq!(pos.into(), None);
            self.cb.labels[&handle]
        } else {
            let pos = self.node(pos.into());
            let prev = self.cb.coils.insert(handle.clone(), pos);
            assert_eq!(prev, None);
            let prev = self.cb.labels.insert(handle, pos);
            assert_eq!(prev, None);
            pos
        }
    }

    pub fn add_switch(&mut self, name: impl Into<Handle>, loc: (impl Into<Option<NodeId>>, impl Into<Option<NodeId>>, impl Into<Option<NodeId>>)) -> (NodeId, NodeId, NodeId) {
        let pole = self.node(loc.0.into());
        let no = self.node(loc.1.into());
        let nc = self.node(loc.2.into());
        self.cb.switches.push(BuilderSwitch { name: name.into(), pole, no, nc });
        (pole, no, nc)
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

impl Circuit {

    /// Signal that a node has been pulled high and propagate effects through the circuit
    ///
    /// # Arguments
    ///
    /// * `node_name` - Alias for the node being pulled high
    pub fn step_a(&mut self) -> Vec<bool> {
        let mut visited = vec![false; self.num_nodes];
        
        let mut next_switch_positions = vec![false; self.switches.len()];
        if self.initialized {
            self.sources.push(self.labels[&handle!("G")]);
            while let Some(node) = self.sources.pop() {
                if visited[node] {
                    continue;
                }
                visited[node] = true;

                for coil in &self.coils[node] {
                    for switch in &coil.switches {
                        next_switch_positions[*switch] = true;
                    }
                }
                for other in &self.connections[node] {
                    if !visited[*other] {
                        self.sources.push(*other);
                    }
                }
            }
            for (node_id, b) in &mut self.traces {
                *b = visited[*node_id];
            }
        }
        next_switch_positions
    }

    pub fn step_b(&mut self, mut next_switch_positions: Vec<bool>) {
        std::mem::swap(&mut self.switch_positions, &mut next_switch_positions);
        self.connections = vec![Vec::new(); self.num_nodes];
        for (active, switch) in zip(&self.switch_positions, &self.switches) {
            let branch = if *active {
                switch.no
            } else {
                switch.nc
            };
            Circuit::connect(&mut self.connections, switch.pole, branch);
        }
    }

    pub fn step(&mut self) {
        let next_switch_positions = self.step_a();
        self.step_b(next_switch_positions);
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

    use super::*;

    #[test]
    fn empty() {
        let cb = CircuitBuilder::new();
        let mut c = cb.finalize();
        c.step();
    }

    #[test]
    fn one_relay() {
        let (mut no, mut nc) = (0usize, 0usize);
        let mut c = CircuitBuilder::new().add_subcircuit(|mut scb| {
            let g = scb.label("G");
            scb.add_coil(handle!("Ab", 0), g);
            (_, no, nc) = scb.add_switch("ab_0", (g, None, None));
            scb.trace_all([no, nc]);
        }).finalize();
        assert_eq!((c.switch_positions[0], c.traces[&no], c.traces[&nc]), (false, false, false));
        c.step(); // turn on
        assert_eq!((c.switch_positions[0], c.traces[&no], c.traces[&nc]), (true, false, true));
        c.step();
        assert_eq!((c.switch_positions[0], c.traces[&no], c.traces[&nc]), (true, true, false));
    }

    #[test]
    fn oscillating_relay() {
        let mut coil_node = 0usize;
        let mut c = CircuitBuilder::new().add_subcircuit(|mut scb| {
            let g = scb.label("G");
            (_, _, coil_node) = scb.add_switch("xy_-10", (g, None, None));
            scb.add_coil("Xy_-10", Some(coil_node));
            scb.trace(coil_node);
        }).finalize();
        c.step();
        for _ in 0..5 {
            assert_eq!((c.switch_positions[0], c.traces[&coil_node]), (true, true));
            c.step();
            assert_eq!((c.switch_positions[0], c.traces[&coil_node]), (false, false));
            c.step();
        }
    }

    #[test]
    fn step_subcircuit() {
        let mut step = [0usize; 6]; // step[0] is unused for simplicity
        let mut step123 = 0usize;
        let mut c = CircuitBuilder::new().add_subcircuit(|mut scb| {
            let g = scb.label("G");
            step123 = scb.label("step123");
            scb.trace(step123);
            scb.add_coil("Init", g);
            for i in 1..=5 {
                step[i] = scb.add_coil(handle!("S", i as i8), None);
                scb.trace(step[i]);
            }
            scb.add_switch("init", (g, None, step[1]));
            scb.add_switch(handle!("s", 1), (g, step[2], None));
            scb.add_switch(handle!("s", 2), (g, step[3], None));
            scb.add_switch(handle!("s", 3), (g, step[4], None));
            scb.add_switch(handle!("s", 4), (g, step[5], None));
            scb.add_switch(handle!("s", 5), (g, step[1], None));

            scb.add_switch("init", (g, None, step123));
            scb.add_switch(handle!("s", 5), (g, step123, None));
            scb.add_switch(handle!("s", 1), (g, step123, None));
            scb.add_switch(handle!("s", 2), (g, step123, None));
        }).finalize();

        let test = |c: &Circuit, expected_step: usize| {
            let mut expected_states = [false; 5];
            if expected_step != 0 {
                expected_states[expected_step - 1] = true;
            }
            for (i, expected) in expected_states.into_iter().enumerate() {
                assert_eq!(expected, c.traces[&step[i + 1]]);
            }
        };

        test(&c, 0);
        for _ in 0..3 {
            for s in 1..=5 {
                c.step();
                test(&c, s);
                assert_eq!(s == 1 || s == 2 || s == 3, c.traces[&step123]);
            }
        }
    }

    #[test]
    fn chain_alternating_relays() {
        let mut c = CircuitBuilder::new()
            .add_subcircuit(|mut scb| {
                let g = scb.label("G");
                let (last_a, last_b) = SubcircuitBuilder::chain((g, g), 0..5, |(left_a, left_b), i| {
                    scb.add_coil(handle!("Bb", i), Some(left_a));
                    let right_a = scb.label(handle!("a", i));
                    let right_b = scb.label(handle!("b", i));
                    scb.trace_all([right_a, right_b]);
                    scb.add_switch(
                        handle!("aa", i),
                        (Some(left_a), Some(right_a), None));
                    scb.add_switch(
                        handle!("bb", i),
                        (Some(left_b), Some(right_b), None));
                    scb.add_coil(handle!("Aa", i), Some(right_b));
                    (right_a, right_b)
                });
                assert_eq!(last_a, scb.label("a_4"));
                assert_eq!(last_b, scb.label("b_4"));
            })
            .finalize();
        let test = |c: &Circuit, expected_a: i8, expected_b: i8| {
            for i in 0..5 {
                assert_eq!(c.traces[&c.labels[&handle!("a", i)]], i < expected_a);
                assert_eq!(c.traces[&c.labels[&handle!("b", i)]], i < expected_b);
            }
        };

        c.step();
        test(&c, 0, 0);
        for i in 0..5 {
            c.step();
            test(&c, i, i + 1);
            c.step();
            test(&c, i + 1, i + 1);
        }
    }

}
