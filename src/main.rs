use crate::circuit::{Circuit, CBuilder, NodeSpec::*, Interface};

pub mod circuit;

fn main() {
    env_logger::init();
    let mut c = Circuit::new();

    // Figure 4
    c.build_subcircuit("Additionswerk", |builder| {
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
            let bc = builder.add_coil(&format!("Bc{}", i), New);
            let [_, ba_no, ba_nc] = builder.add_switch(&format!("ba{}", i), [Wire(bc), New, New]);
            builder.add_switch(&format!("bb{}", i), [Wire(b1), Wire(ba_nc), Wire(ba_no)]);
            builder.add_switch(&format!("bb{}", i), [Wire(b2), Wire(ba_no), Wire(ba_nc)]);
        }
        let [br_pole, _, _] = builder.add_switch("br", [New, Named("a60"), New]);
        let [_, _, ba2_nc] = builder.add_switch("ba2", [Named("b61"), Named("II_III"), New]);
        let [_, b4, b3] = builder.add_switch("bs", [Named("II_III"), Named("b60"), New]);
        CBuilder::chain(ba2_nc, (-16..=1).rev(), |left, i| {
            let right = builder.add_coil(&format!("Bd{}", i), New);
            let bb_pole = builder.node(New);
            builder.add_switch(&format!("bb{}", i), [Wire(bb_pole), Wire(b3), Wire(b4)]);
            builder.add_switch(&format!("ba{}", i), [Wire(left), Wire(bb_pole), New]);
            builder.add_switch(&format!("bc{}", i), [Wire(right), Wire(left), New]);
            right
        });
        builder.add_switch("ba1", [Wire(br_pole), Named("II_III"), Named("Bd1")]);

        builder.add_coil("Br", New);

        let s3 = builder.node(Named("III"));
        for i in (-16..=1).rev() {
            let be = builder.add_coil(&format!("Be{}", i), New);
            if i == 1 {
                let be1p = builder.add_coil("Be'1", New);
                builder.add_switch("br", [Wire(be), Wire(be1p), New]);
            }
            let [_, bt_no, bt_nc] = builder.add_switch("bt", [Wire(be), New, New]);
            let [_, bd_no, bd_nc] = builder.add_switch(&format!("bd{}", i), [Wire(bt_no), New, New]);
            builder.add_switch(&format!("bc{}", i), [Wire(s3), Wire(bd_nc), Wire(bd_no)]);
            builder.add_switch(&format!("ba{}", i), [Wire(s3), Wire(bt_nc), New]);
        }
        interface
    });
    c.step();
}

