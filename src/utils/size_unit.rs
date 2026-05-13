use std::str::FromStr;

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum SizeUnit {
    B,
    KB,
    MB,
    #[default]
    GB,
    TB,
}

impl SizeUnit {
    pub fn multiplier(&self) -> u64 {
        match self {
            SizeUnit::B => 1,
            SizeUnit::KB => 1024,
            SizeUnit::MB => 1024 * 1024,
            SizeUnit::GB => 1024 * 1024 * 1024,
            SizeUnit::TB => 1024 * 1024 * 1024 * 1024,
        }
    }

    pub fn all() -> [SizeUnit; 5] {
        [
            SizeUnit::B,
            SizeUnit::KB,
            SizeUnit::MB,
            SizeUnit::GB,
            SizeUnit::TB,
        ]
    }
}

impl std::fmt::Display for SizeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SizeUnit::B => write!(f, "B"),
            SizeUnit::KB => write!(f, "KB"),
            SizeUnit::MB => write!(f, "MB"),
            SizeUnit::GB => write!(f, "GB"),
            SizeUnit::TB => write!(f, "TB"),
        }
    }
}

impl FromStr for SizeUnit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "B" => Ok(SizeUnit::B),
            "KB" => Ok(SizeUnit::KB),
            "MB" => Ok(SizeUnit::MB),
            "GB" => Ok(SizeUnit::GB),
            "TB" => Ok(SizeUnit::TB),
            _ => Err(()),
        }
    }
}
