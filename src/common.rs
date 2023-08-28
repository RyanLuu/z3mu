use super::circuit::{CBuilder, Interface, NodeSpec::*, Handle};

pub fn gate<'a, I: Iterator<Item = i8> + 'a>(from: &'a str, gate: &'a str, to: &'a str, indices: I) -> impl FnOnce(&mut CBuilder) -> Interface + 'a {
    move |builder: &mut CBuilder| {
        builder.add_coil(gate, New);
        let s5 = builder.node("S5");
        for index in indices {
            let (_, from_no, _) = builder.add_switch(CBuilder::coil_to_switch_name(&handle!(from, index)), (s5, New, New));
            let (_, gate_no, _) = builder.add_switch(CBuilder::coil_to_switch_name(&handle!(gate)), (from_no, New, New));
            builder.add_coil(handle!(to, index), gate_no);
        }
        Interface::new([gate, "S5"])
    }
}

