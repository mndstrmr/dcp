#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Size64,
    Size32,
    Size16,
    Size8,
}

impl Size {
    pub fn byte_count(&mut self) -> usize {
        match self {
            Size::Size64 => 8,
            Size::Size32 => 4,
            Size::Size16 => 2,
            Size::Size8 => 1,
        }
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Size::Size64 => f.write_str("q"),
            Size::Size32 => f.write_str("d"),
            Size::Size16 => f.write_str("w"),
            Size::Size8 => f.write_str("b"),
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Unknown64,
    Unknown32,
    Unknown16,
    Unknown8,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Unknown64 => f.write_str("u64"),
            Ty::Unknown32 => f.write_str("u32"),
            Ty::Unknown16 => f.write_str("u16"),
            Ty::Unknown8 => f.write_str("u8"),
        }
    }
}
