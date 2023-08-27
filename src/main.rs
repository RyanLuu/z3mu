use crate::circuit::{Circuit, CBuilder, NodeSpec::*, Interface};

pub mod circuit;

fn main() {
    env_logger::init();
    let mut c = Circuit::new();

    // Figure 4
    // Adds mantissas Ba and Bb and stores the sum in Be
    c.build_subcircuit("Additionswerk (Teil B für Mantisse)", |builder| {
        let mut interface = Interface::new(&["a60", "b60", "b61", "Br"]);
        for i in -16..=2 {
            let coil_name = format!("Ba{}", i);
            builder.add_coil(&coil_name, New);
            interface.push(coil_name);
        }
        for i in -16..=1 {
            let coil_name = format!("Bb{}^1", i);
            builder.add_coil(&coil_name, New);
            interface.push(coil_name);
        }

        let [_, b2, b1] = builder.add_switch("bs", [Named("I_II_III"), New, New]);
        builder.add_coil("Bs", New);
        for i in (-16..=1).rev() {
            let bc = builder.add_coil(format!("Bc{}", i), New);
            let [_, ba_no, ba_nc] = builder.add_switch(format!("ba{}", i), [Wire(bc), New, New]);
            builder.add_switch(format!("bb{}", i), [Wire(b1), Wire(ba_nc), Wire(ba_no)]);
            builder.add_switch(format!("bb{}", i), [Wire(b2), Wire(ba_no), Wire(ba_nc)]);
        }
        let [br_pole, _, _] = builder.add_switch("br", [New, Named("a60"), New]);
        let [_, _, ba2_nc] = builder.add_switch("ba2", [Named("b61"), Named("II_III"), New]);
        let [_, b4, b3] = builder.add_switch("bs", [Named("II_III"), Named("b60"), New]);
        CBuilder::chain(ba2_nc, (-16..=1).rev(), |left, i| {
            let right = builder.add_coil(format!("Bd{}", i), New);
            let bb_pole = builder.node(New);
            builder.add_switch(format!("bb{}", i), [Wire(bb_pole), Wire(b3), Wire(b4)]);
            builder.add_switch(format!("ba{}", i), [Wire(left), Wire(bb_pole), New]);
            builder.add_switch(format!("bc{}", i), [Wire(right), Wire(left), New]);
            right
        });
        builder.add_switch("ba1", [Wire(br_pole), Named("II_III"), Named("Bd1")]);

        builder.add_coil("Br", New);

        let s3 = builder.node(Named("III"));
        for i in (-16..=1).rev() {
            let be = builder.add_coil(format!("Be{}", i), New);
            if i == 1 {
                let be1p = builder.add_coil("Be'1", New);
                builder.add_switch("br", [Wire(be), Wire(be1p), New]);
            }
            let [_, bt_no, bt_nc] = builder.add_switch("bt", [Wire(be), New, New]);
            let [_, bd_no, bd_nc] = builder.add_switch(format!("bd{}", i), [Wire(bt_no), New, New]);
            builder.add_switch(format!("bc{}", i), [Wire(s3), Wire(bd_nc), Wire(bd_no)]);
            builder.add_switch(format!("ba{}", i), [Wire(s3), Wire(bt_nc), New]);
        }
        interface
    });

    // Figure 5
    // Copies Af into Aa upon activating Ea
    c.build_subcircuit("Kontakte der E-Relais", |builder| {
        let v = builder.node(Named("V"));
        for i in (0..=6).rev() {
            let [_, af_no, _] = builder.add_switch(format!("af{}", i), [Wire(v), New, New]);
            let [_, ea_no, _] = builder.add_switch("ea", [Wire(af_no), New, New]);
            builder.add_coil(format!("Aa{}", i), Wire(ea_no));
        }
        builder.add_coil("Ea", New);
        Interface::new(&["Ea"])
    });

    // Figure 6
    // Shifts input into Ba by -2Fp + Fq bits
    c.build_subcircuit("Kontakte der Relais Fp, Fq", |builder| {
        let mut interface = Interface::new(&[]);
        let (_, left2, prev_coil) = CBuilder::chain((New, New, New), (-16..=1).rev(), |(left1, left2, prev_coil), i| {
            let input_name = if i == 0 { "0".into() } else { format!("{:+}", i) };
            let input = builder.node(Named(&input_name));
            interface.push(input_name);
            let [_, fp_no, fp_nc] = builder.add_switch("fp", [Wire(input), New, left2]);
            let [_, _, fq_nc] = builder.add_switch("fq", [Wire(fp_nc), prev_coil, New]);
            let coil_name = format!("Ba{}", i);
            let coil = builder.add_coil(&coil_name, Wire(fq_nc));
            interface.push(coil_name);
            (Wire(fp_no), left1, Wire(coil))
        });
        builder.add_switch("fq", [left2, prev_coil, New]);
        interface
    });

    c.step();
}

