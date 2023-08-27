use crate::circuit::{Circuit, NodeSpec::*};

pub mod circuit;

fn main() {
    env_logger::init();
    let mut c = Circuit::new();

    // Figure 4
    c.build_subcircuit("Additionswerk", vec!["a60", "b60"], |builder| {
        for i in -16..=2 {
            builder.add_coil(&format!("Ba{}", i), New);
        }
        for i in -16..=1 {
            builder.add_coil(&format!("Bb{}^1", i), New);
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
        builder.add_coil("Br", New);
        let [_, b4, b3] = builder.add_switch("bs", [Named("II_III"), Named("b60"), New]);
        for i in (-16..=1).rev() {
            let bd = builder.add_coil(&format!("Bd{}", i), New);
            let bb_pole = builder.add_node();
            let ba_pole = if i == 1 {
                ba2_nc
            } else {
                builder.get_node(Named(&format!("Bd{}", i + 1)))
            };
            builder.add_switch(&format!("bb{}", i), [Wire(bb_pole), Wire(b3), Wire(b4)]);
            builder.add_switch(&format!("ba{}", i), [Wire(ba_pole), Wire(bb_pole), New]);
            builder.add_switch(&format!("bc{}", i), [Wire(bd), Wire(ba_pole), New]);
        }
        builder.add_switch("ba1", [Wire(br_pole), Named("II_III"), Named("Bd1")]);

        let s3 = builder.get_node(Named("III"));
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
    });
    c.step();
}
