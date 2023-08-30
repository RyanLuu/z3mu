use super::circuit::{CBuilder, Interface, NodeSpec::*, Handle};

pub fn gate<'a, I: Iterator<Item = i8> + 'a>(from: &'a str, gate: &'a str, to: &'a str, indices: I) -> impl FnOnce(&mut CBuilder) -> Interface + 'a {
    move |builder: &mut CBuilder| {
        builder.add_coil(gate, New);
        let s5 = builder.node("S_5");
        for index in indices {
            let (_, from_no, _) = builder.add_switch(CBuilder::coil_to_switch_name(&handle!(from, index)), (s5, New, New));
            let (_, gate_no, _) = builder.add_switch(CBuilder::coil_to_switch_name(&handle!(gate)), (from_no, New, New));
            builder.add_coil(handle!(to, index), gate_no);
        }
        Interface::new([gate])
    }
}

pub fn gate_const<'a, I: Iterator<Item = i8> + 'a>(k: i8, gate: Handle, to: &'a str, indices: I) -> impl FnOnce(&mut CBuilder) -> Interface + 'a {
    move |builder: &mut CBuilder| {
        builder.add_coil(gate.clone(), New);
        let s5 = builder.node("S_5");
        for index in indices {
            assert!(index >= 0 && index < 8); // just supports i8 for now
            if (k >> index) & 1 != 0 {
                let (_, gate_no, _) = builder.add_switch(CBuilder::coil_to_switch_name(&gate), (s5, New, New));
                builder.add_coil(handle!(to, index), gate_no);
            }
        }
        Interface::new([gate])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;
    use crate::circuit::Bus;

    #[test]
    fn gate_const_test() {
        let mut c = Circuit::new();
        c.build_subcircuit("A", gate_const(-123i8, handle!("Ga"), "Aa", 0..=7));

        c.set(&handle!("S", 5));
        c.step();
        assert_eq!(c.inspect_bus(&bus!("Aa")), 0);

        c.set(&handle!("Ga"));
        c.step();
        c.set(&handle!("S", 5));
        c.step();
        assert_eq!(c.inspect_bus(&bus!("Aa")), -123);
    }
}

