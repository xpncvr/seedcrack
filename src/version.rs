#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Version {
    V1_21_11,
    V26_1_1,
    V26_2,
}

impl Version {
    pub const ALL: [Version; 3] = [Version::V1_21_11, Version::V26_1_1, Version::V26_2];
    pub const LATEST: Version = Version::V26_2;

    pub const fn protocol(self) -> i32 {
        match self {
            Version::V1_21_11 => 774,
            Version::V26_1_1 => 775,
            Version::V26_2 => 776,
        }
    }

    pub fn from_protocol(protocol: i32) -> Option<Version> {
        Version::ALL.into_iter().find(|v| v.protocol() == protocol)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Version::V1_21_11 => "1.21.11",
            Version::V26_1_1 => "26.1.1",
            Version::V26_2 => "26.2",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenRange {
    pub min_y: i32,
    pub height: i32,
}

impl GenRange {
    pub const OVERWORLD: GenRange = GenRange { min_y: -64, height: 384 };

    pub const fn top(self) -> i32 {
        self.min_y + self.height - 1
    }
}

impl Default for GenRange {
    fn default() -> Self {
        GenRange::OVERWORLD
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}
