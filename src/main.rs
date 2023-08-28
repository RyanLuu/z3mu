use crate::circuit::{Circuit, CBuilder, NodeSpec::*, Interface, Handle};

#[macro_use]
pub mod circuit;

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
    c.build_subcircuit("Kontakte der E-Relais", |builder| {
        let s5 = builder.node("S5");
        for i in (0..=6).rev() {
            let (_, af_no, _) = builder.add_switch(handle!("af", i), (Wire(s5), New, New));
            let (_, ea_no, _) = builder.add_switch("ea", (Wire(af_no), New, New));
            builder.add_coil(handle!("Aa", i), Wire(ea_no));
        }
        builder.add_coil("Ea", New);
        Interface::new(["Ea"])
    });

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

    c.step();
    c.inspect(&handle!("Br"));
    c.inspect_all("Ba");
}

