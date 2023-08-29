use crate::circuit::{Circuit, CBuilder, NodeSpec::*, Interface, Handle};

#[macro_use]
pub mod circuit;
pub mod common;

fn main() {
    env_logger::init();
    let mut c = Circuit::new();

    // Figure 4
    // Adds mantissas Ba and Bb and stores the sum in Be
    c.build_subcircuit("Additionswerk (Teil B für Mantisse)", |builder| {
        let mut interface = Interface::new(["a_60", "b_60", "b_61", "Br"]);
        for coil_handle in (-16..=2).map(|i| handle!("Ba", i)) {
            builder.add_coil(coil_handle.clone(), New);
            interface.push(coil_handle);
        }
        for coil_handle in (-16..=1).map(|i| handle!("Bb", i, 1)) {
            builder.add_coil(coil_handle.clone(), New);
            interface.push(coil_handle);
        }

        let (_, b2, b1) = builder.add_switch("bs", ("S123", (), ()));
        builder.add_coil("Bs", New);
        for i in (-16..=1).rev() {
            let bc = builder.add_coil(handle!("Bc", i), New);
            let (_, ba_no, ba_nc) = builder.add_switch(handle!("ba", i), (Wire(bc), New, New));
            builder.add_switch(handle!("bb", i), (Wire(b1), Wire(ba_nc), Wire(ba_no)));
            builder.add_switch(handle!("bb", i), (Wire(b2), Wire(ba_no), Wire(ba_nc)));
        }
        let (br_pole, _, _) = builder.add_switch("br", (New, "a_60", New));
        let (_, _, ba2_nc) = builder.add_switch("ba_2", ("b_61", "S23", New));
        let (_, b4, b3) = builder.add_switch("bs", ("S23", "b_60", New));
        CBuilder::chain(ba2_nc, (-16..=1).rev(), |left, i| {
            let right = builder.add_coil(format!("Bd{}", i), New);
            let bb_pole = builder.node(New);
            builder.add_switch(handle!("bb", i), (Wire(bb_pole), Wire(b3), Wire(b4)));
            builder.add_switch(handle!("ba", i), (Wire(left), Wire(bb_pole), New));
            builder.add_switch(handle!("bc", i), (Wire(right), Wire(left), New));
            right
        });
        builder.add_switch("ba_1", (Wire(br_pole), "S23", "Bd_1"));

        builder.add_coil("Br", New);

        let s3 = builder.node("S3");
        for i in (-16..=1).rev() {
            let be = builder.add_coil(handle!("Be", i), New);
            if i == 1 {
                let be1p = builder.add_coil("Be'_1", New);
                builder.add_switch("br", (Wire(be), Wire(be1p), New));
            }
            let (_, bt_no, bt_nc) = builder.add_switch("bt", (Wire(be), New, New));
            let (_, bd_no, bd_nc) = builder.add_switch(handle!("bd", i), (Wire(bt_no), New, New));
            builder.add_switch(handle!("bc", i), (Wire(s3), Wire(bd_nc), Wire(bd_no)));
            builder.add_switch(handle!("ba", i), (Wire(s3), Wire(bt_nc), New));
        }
        interface
    });

    // Figure 5
    // Copies Af into Aa upon activating Ea
    c.build_subcircuit("Kontakte der E-Relais (Ea)", common::gate("Af", "Ea", "Aa", 0..=6));
    c.build_subcircuit("Kontakte der E-Relais (Eb)", common::gate("Af", "Ea", "Ab", 0..=6));
    // TODO: shifted gate Ee
    c.build_subcircuit("Kontakte der E-Relais (Ec)", common::gate("Ae", "Ec", "Aa", 0..=7));
    c.build_subcircuit("Kontakte der E-Relais (Ed)", common::gate("Ae", "Ed", "Ab", 0..=7));
    // TODO: wire Fa into Fpq shifter
    // TODO: wire Fb into Fhiklm shifter
    // TODO: wire Fc into Fpq shifter
    // TODO: wire Fd into Fhiklm shifter
    c.build_subcircuit("Kontakte der E-Relais (Ff)", common::gate("Be", "Ff", "Bf", -16..=0));
    // TODO: shifted gate Be'_1
    // TODO: read input into Ba using Zabcd
    c.build_subcircuit("Kontakte der E-Relais (Ei)", common::gate_const(-4i8, "Ei".into(), "Ab", 0..=7));
    c.build_subcircuit("Kontakte der E-Relais (Eh)", common::gate_const( 3i8, "Eh".into(), "Ab", 0..=7));
    c.build_subcircuit("Kontakte der E-Relais (Eg)", common::gate_const(13i8, "Eg".into(), "Aa", 0..=7));

    // Figure 6
    // Shifts input into Ba by -2Fp + Fq bits
    c.build_subcircuit("Kontakte der Relais Fp, Fq", |builder| {
        let mut interface = Interface::empty();
        let (_, left2, prev_coil) = CBuilder::chain((New, New, New), (-16..=1).rev(), |(left1, left2, prev_coil), i| {
            let input_name = if i == 0 { "0".into() } else { format!("{:+}", i) };
            let input = builder.node(input_name.as_str());
            interface.push(input_name);
            let (_, fp_no, fp_nc) = builder.add_switch("fp", (Wire(input), New, left2));
            let (_, _, fq_nc) = builder.add_switch("fq", (Wire(fp_nc), prev_coil, New));
            let coil_name = handle!("Ba", i);
            let coil = builder.add_coil(coil_name.clone(), Wire(fq_nc));
            interface.push(coil_name);
            (Wire(fp_no), left1, Wire(coil))
        });
        builder.add_switch("fq", (left2, prev_coil, New));
        interface
    });

    // Figure 7
    // Shifts input into Bb by -16Fh + 8Fi + 4Fk + 2Fl + Fm bits
    c.build_subcircuit("Kontakte der Relais Fh, Fi, Fk, Fl, Fm", |builder| {
        let mut interface = Interface::empty();
        let fh_nodes: [_; 18] = (-16..=1).rev().map(|i| {
            let input_name = if i == 0 { "0".into() } else { format!("{:+}", i) };
            let input = builder.node(input_name.as_str());
            interface.push(input_name);
            input
        }).collect::<Vec<_>>().try_into().unwrap();
        let fi_nodes: [_; 33] = std::array::from_fn(|_| builder.node(New));
        let fk_nodes: [_; 25] = std::array::from_fn(|_| builder.node(New));
        let fl_nodes: [_; 21] = std::array::from_fn(|_| builder.node(New));
        let fm_nodes: [_; 19] = std::array::from_fn(|_| builder.node(New));
        let bb_nodes: [_; 18] = std::array::from_fn(|i| {
            let coil_handle = handle!("Bb", 1 - i as i8);
            interface.push(coil_handle.clone());
            builder.add_coil(coil_handle, New)
        });
        for (i, node) in fh_nodes.into_iter().enumerate() {
            let no = if i < 17 { fi_nodes[i + 16] } else { builder.node(New) };
            builder.add_switch("fh", (node, no, fi_nodes[i]));
        }
        for (i, node) in fi_nodes.into_iter().enumerate() {
            let no = if i >= 8 { fk_nodes[i - 8] } else { builder.node(New) };
            let nc = if i < fk_nodes.len() { fk_nodes[i] } else { builder.node(New) };
            builder.add_switch("fi", (node, no, nc));
        }
        for (i, node) in fk_nodes.into_iter().enumerate() {
            let no = if i >= 4 { fl_nodes[i - 4] } else { builder.node(New) };
            let nc = if i < fl_nodes.len() { fl_nodes[i] } else { builder.node(New) };
            builder.add_switch("fk", (node, no, nc));
        }
        for (i, node) in fl_nodes.into_iter().enumerate() {
            let no = if i >= 2 { fm_nodes[i - 2] } else { builder.node(New) };
            let nc = if i < fm_nodes.len() { fm_nodes[i] } else { builder.node(New) };
            builder.add_switch("fl", (node, no, nc));
        }
        for (i, node) in fm_nodes.into_iter().enumerate() {
            let no = if i > 0 { bb_nodes[i - 1] } else { builder.node(New) };
            let nc = if i < bb_nodes.len() { bb_nodes[i] } else { builder.node(New) };
            builder.add_switch("fm", (node, no, nc));
        }

        interface
    });

    c.set(&handle!("Ei"));
    c.step();
    c.set(&handle!("S", 5));
    c.step();
    c.inspect_bus("Ab");
}

