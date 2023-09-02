#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct Handle {
    pub name: String,
    pub index: Option<i8>,
    pub sup: Option<u8>,
}

impl Handle {
    pub fn new<T: Into<String>>(name: T, index: Option<i8>, sup: Option<u8>) -> Self {
        let name = name.into();
        assert!(!name.contains('_'));
        assert!(!name.contains('^'));
        Handle { name, index, sup }
    }
}

impl std::fmt::Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(index) = self.index {
            write!(f, "_{}", index)?;
        }
        if let Some(sup) = self.sup {
            write!(f, "^{}", sup)?;
        }
        Ok(())
    }
}

impl From<&str> for Handle {
    fn from(s: &str) -> Handle {
        let mut rem: &str = s;
        let sup = rem.find('^').map(|i| {
            let sup = rem[i+1..].parse().unwrap();
            rem = &rem[..i];
            sup
        });
        let index = rem.find('_').map(|i| {
            let index = rem[i+1..].parse().expect(&format!("Failed to parse handle {}", s));
            rem = &rem[..i];
            index
        });
        Handle {
            name: rem.into(),
            index,
            sup,
        }
    }
}

impl From<String> for Handle {
    fn from(s: String) -> Handle {
        Handle::from(s.as_str())
    }
}


macro_rules! handle {
    ( $name:expr ) => {
        Handle::new($name, None, None)
    };
    ( $name:expr, $index:expr ) => {
        Handle::new($name, Some($index), None)
    };
    ( $name:expr, $index:expr, $sup:expr ) => {
        Handle::new($name, Some($index), Some($sup))
    };
}

pub struct Bus {
    pub name: String,
    pub sup: Option<u8>,
}

impl Bus {
    pub fn new<T: Into<String>>(name: T, sup: Option<u8>) -> Self {
        let name = name.into();
        assert!(!name.contains('_'));
        assert!(!name.contains('^'));
        Bus { name, sup }
    }

    pub fn index(&self, index: i8) -> Handle {
        Handle::new(self.name.clone(), Some(index), self.sup)
    }
}

impl std::fmt::Display for Bus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(sup) = self.sup {
            write!(f, "^{}", sup)?;
        }
        Ok(())
    }
}

macro_rules! bus {
    ( $name:expr ) => {
        Bus::new($name, None)
    };
    ( $name:expr, $sup:expr ) => {
        Bus::new($name, Some($sup))
    };
}

