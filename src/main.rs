use crate::circuit::{SubcircuitBuilder, CircuitBuilder, Handle, Bus, NodeId};

#[macro_use]
pub mod circuit;
pub mod common;

fn main() {
    env_logger::init();

    // Figure 4
    // Adds mantissas Ba and Bb and stores the sum in Be
    let figure4 = |mut scb: SubcircuitBuilder| {
        for coil_handle in (-16..=2).map(|i| handle!("Ba", i)) {
            scb.add_coil(coil_handle.clone(), None);
        }
        for coil_handle in (-16..=1).map(|i| handle!("Bb", i, 1)) {
            scb.add_coil(coil_handle.clone(), None);
        }

        let s123 = scb.label(handle!("S", 123));
        let (_, b2, b1) = scb.add_switch("bs", (s123, None, None));
        scb.add_coil("Bs", None);
        for i in (-16..=1).rev() {
            let bc = scb.add_coil(handle!("Bc", i), None);
            let (_, ba_no, ba_nc) = scb.add_switch(handle!("ba", i), (bc, None, None));
            scb.add_switch(handle!("bb", i), (b1, ba_nc, ba_no));
            scb.add_switch(handle!("bb", i), (b2, ba_no, ba_nc));
        }
        let a60 = scb.label(handle!("a", 60));
        let b60 = scb.label(handle!("b", 60));
        let b61 = scb.label(handle!("b", 61));
        let s23 = scb.label(handle!("S", 23));
        let (br_pole, _, _) = scb.add_switch("br", (None, a60, None));
        let (_, _, ba2_nc) = scb.add_switch("ba_2", (b61, s23, None));
        let (_, b4, b3) = scb.add_switch("bs", (s23, b60, None));
        SubcircuitBuilder::chain(ba2_nc, (-16..=1).rev(), |left, i| {
            let right = scb.add_coil(format!("Bd{}", i), None);
            let bb_pole = scb.node(None);
            scb.add_switch(handle!("bb", i), (bb_pole, b3, b4));
            scb.add_switch(handle!("ba", i), (left, bb_pole, None));
            scb.add_switch(handle!("bc", i), (right, left, None));
            right
        });
        let bd1 = scb.label(handle!("Bd", 1));
        scb.add_switch("ba_1", (br_pole, s23, bd1));

        scb.add_coil("Br", None);

        let s3 = scb.label(handle!("S", 3));
        for i in (-16..=1).rev() {
            let be = scb.add_coil(handle!("Be", i), None);
            if i == 1 {
                let be1p = scb.add_coil("Be'_1", None);
                scb.add_switch("br", (be, be1p, None));
            }
            let (_, bt_no, bt_nc) = scb.add_switch("bt", (be, None, None));
            let (_, bd_no, bd_nc) = scb.add_switch(handle!("bd", i), (bt_no, None, None));
            scb.add_switch(handle!("bc", i), (s3, bd_nc, bd_no));
            scb.add_switch(handle!("ba", i), (s3, bt_nc, None));
        }
    };

    // Figure 5
    // Copies Af into Aa upon activating Ea
    let figure5a = common::gate(bus!("Af"), handle!("Ea"), bus!("Aa"), 0..=6);
    let figure5b = common::gate(bus!("Af"), handle!("Eb"), bus!("Ab"), 0..=6);
    // TODO: shifted gate Ee
    let figure5d = common::gate(bus!("Ae"), handle!("Ec"), bus!("Aa"), 0..=7);
    let figure5e = common::gate(bus!("Ae"), handle!("Ed"), bus!("Ab"), 0..=7);
    // TODO: wire Fa into Fpq shifter
    // TODO: wire Fb into Fhiklm shifter
    // TODO: wire Fc into Fpq shifter
    // TODO: wire Fd into Fhiklm shifter
    let figure5j = common::gate(bus!("Be"), handle!("Ff"), bus!("Bf"), -16..=0);
    // TODO: shifted gate Be'_1
    // TODO: read input into Ba using Zabcd
    let figure5m = common::gate_const(-4i8, handle!("Ei"), bus!("Ab"), 0..=7);
    let figure5n = common::gate_const( 3i8, handle!("Eh"), bus!("Ab"), 0..=7);
    let figure5o = common::gate_const(13i8, handle!("Eg"), bus!("Aa"), 0..=7);

    // Figure 6
    // Shifts input into Ba by -2Fp + Fq bits
    let figure6 = |mut scb: SubcircuitBuilder| {
        let (_, left2, prev_coil) = SubcircuitBuilder::chain((None, None, None), (-16..=1).rev(), |(left1, left2, prev_coil), i| {
            let input_name = if i == 0 { "0".into() } else { format!("{:+}", i) };
            let input = scb.label(input_name.as_str());
            let (_, fp_no, fp_nc) = scb.add_switch("fp", (input, None, left2));
            let coil = scb.add_coil(handle!("Ba", i), None);
            scb.add_switch("fq", (fp_nc, prev_coil, coil));
            (Some(fp_no), left1, Some(coil))
        });
        scb.add_switch("fq", (left2, prev_coil, None));
    };

    // Figure 7
    // Shifts input into Bb by -16Fh + 8Fi + 4Fk + 2Fl + Fm bits
    let figure7 = |mut scb: SubcircuitBuilder| {
        let fh_nodes: [_; 18] = (-16..=1).rev().map(|i| {
            let input_name = if i == 0 { "0".into() } else { format!("{:+}", i) };
            let input = scb.label(input_name.as_str());
            input
        }).collect::<Vec<_>>().try_into().unwrap();
        let fi_nodes: [_; 33] = std::array::from_fn(|_| scb.node(None));
        let fk_nodes: [_; 25] = std::array::from_fn(|_| scb.node(None));
        let fl_nodes: [_; 21] = std::array::from_fn(|_| scb.node(None));
        let fm_nodes: [_; 19] = std::array::from_fn(|_| scb.node(None));
        let bb_nodes: [_; 18] = std::array::from_fn(|i| {
            let coil_handle = handle!("Bb", 1 - i as i8);
            scb.add_coil(coil_handle, None)
        });
        for (i, node) in fh_nodes.into_iter().enumerate() {
            let no = if i < 17 { fi_nodes[i + 16] } else { scb.node(None) };
            scb.add_switch("fh", (node, no, fi_nodes[i]));
        }
        for (i, node) in fi_nodes.into_iter().enumerate() {
            let no = if i >= 8 { fk_nodes[i - 8] } else { scb.node(None) };
            let nc = if i < fk_nodes.len() { fk_nodes[i] } else { scb.node(None) };
            scb.add_switch("fi", (node, no, nc));
        }
        for (i, node) in fk_nodes.into_iter().enumerate() {
            let no = if i >= 4 { fl_nodes[i - 4] } else { scb.node(None) };
            let nc = if i < fl_nodes.len() { fl_nodes[i] } else { scb.node(None) };
            scb.add_switch("fk", (node, no, nc));
        }
        for (i, node) in fl_nodes.into_iter().enumerate() {
            let no = if i >= 2 { fm_nodes[i - 2] } else { scb.node(None) };
            let nc = if i < fm_nodes.len() { fm_nodes[i] } else { scb.node(None) };
            scb.add_switch("fl", (node, no, nc));
        }
        for (i, node) in fm_nodes.into_iter().enumerate() {
            let no = if i > 0 { bb_nodes[i - 1] } else { scb.node(None) };
            let nc = if i < bb_nodes.len() { bb_nodes[i] } else { scb.node(None) };
            scb.add_switch("fm", (node, no, nc));
        }
    };

    let mut c = CircuitBuilder::new()
        .add_subcircuit(figure4)
        .add_subcircuit(figure5a)
        .add_subcircuit(figure5b)
        .add_subcircuit(figure5d)
        .add_subcircuit(figure5e)
        .add_subcircuit(figure5j)
        .add_subcircuit(figure5m)
        .add_subcircuit(figure5n)
        .add_subcircuit(figure5o)
        .add_subcircuit(figure6)
        .add_subcircuit(figure7)
        .add_subcircuit(|mut scb| {
            let x: Vec<NodeId> = (0..=7).map(|i| scb.label(handle!("Ab", i))).collect();
            scb.trace_all(x);
        })
        .finalize();

    c.set(&handle!("Ei"));
    c.step();
    c.set(&handle!("S", 5));
    c.step();
    c.inspect_bus(&bus!("Ab"));
}

