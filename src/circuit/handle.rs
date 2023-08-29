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

