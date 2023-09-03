use super::circuit::{Handle, Bus, SubcircuitBuilder, CircuitBuilder};

pub fn gate<'a, I: Iterator<Item = i8> + 'a>(from: Bus, gate: Handle, to: Bus, indices: I) -> impl FnOnce(SubcircuitBuilder) {
    move |mut builder: SubcircuitBuilder| {
        builder.add_coil(gate.clone(), None);
        let s5 = builder.label("S_5");
        for index in indices {
            let (_, from_no, _) = builder.add_switch(CircuitBuilder::coil_to_switch_name(&from.index(index)), (s5, None, None));
            let coil_node = builder.add_coil(to.index(index), None);
            builder.add_switch(CircuitBuilder::coil_to_switch_name(&gate), (from_no, coil_node, None));
        }
    }
}

pub fn gate_const<'a, I: Iterator<Item = i8> + 'a>(k: i8, gate: Handle, to: Bus, indices: I) -> impl FnOnce(SubcircuitBuilder) {
    move |mut builder: SubcircuitBuilder| {
        builder.add_coil(gate.clone(), None);
        let s5 = builder.label("S_5");
        for index in indices {
            assert!(index >= 0 && index < (std::mem::size_of_val(&k) * 8) as i8);
            if (k >> index) & 1 != 0 {
                let coil_node = builder.add_coil(to.index(index), None);
                builder.add_switch(CircuitBuilder::coil_to_switch_name(&gate), (s5, coil_node, None));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::Bus;
    
    #[test]
    fn gate_test() {
        let mut c = CircuitBuilder::new()
            .add_subcircuit(gate(bus!("Ab"), handle!("Ga"), bus!("Aa"), 0..=7))
            .add_subcircuit(|mut scb| {
                for i in 0..=7 {
                    let ab = scb.add_coil(handle!("Ab", i), None);
                    let aa = scb.label(handle!("Aa", i));
                    scb.trace_all([ab, aa]);
                }
            })
            .finalize();

        c.set_bus(&bus!("Ab"), -123);
        c.step();
        c.set(&handle!("S", 5));
        c.step();
        assert_eq!(c.inspect_bus(&bus!("Aa")), 0);

        c.set(&handle!("Ga"));
        c.step();
        c.set(&handle!("S", 5));
        c.step();
        assert_eq!(c.inspect_bus(&bus!("Aa")), 0);

        c.set(&handle!("Ga"));
        c.set_bus(&bus!("Ab"), -123);
        c.step();
        c.set(&handle!("S", 5));
        assert_eq!(c.inspect_bus(&bus!("Ab")), -123);
        c.step();
        assert_eq!(c.inspect_bus(&bus!("Aa")), -123);
    }

    #[test]
    fn gate_const_test() {
        let mut c = CircuitBuilder::new()
            .add_subcircuit(gate_const(-123i8, handle!("Ga"), bus!("Aa"), 0..=7))
            .add_subcircuit(|mut scb| {
                for i in 0..=7 {
                    let aa = scb.label(handle!("Aa", i));
                    scb.trace(aa);
                }
            })
            .finalize();

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

