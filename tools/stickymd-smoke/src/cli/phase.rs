//! The phases `phase <id>` and `all` select.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Phase {
    P00,
    P01,
    P02,
    P03,
    P04,
    P05,
    P06,
    P07,
    P08,
    P09,
    P10,
    P11,
    P11B,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    P18,
}

impl Phase {
    pub(crate) const ALL: [Self; 20] = [
        Self::P00,
        Self::P01,
        Self::P02,
        Self::P03,
        Self::P04,
        Self::P05,
        Self::P06,
        Self::P07,
        Self::P08,
        Self::P09,
        Self::P10,
        Self::P11,
        Self::P11B,
        Self::P12,
        Self::P13,
        Self::P14,
        Self::P15,
        Self::P16,
        Self::P17,
        Self::P18,
    ];

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "0" | "00" | "phase-00" => Ok(Self::P00),
            "1" | "01" | "phase-01" => Ok(Self::P01),
            "2" | "02" | "phase-02" => Ok(Self::P02),
            "3" | "03" | "phase-03" => Ok(Self::P03),
            "4" | "04" | "phase-04" => Ok(Self::P04),
            "5" | "05" | "phase-05" => Ok(Self::P05),
            "6" | "06" | "phase-06" => Ok(Self::P06),
            "7" | "07" | "phase-07" => Ok(Self::P07),
            "8" | "08" | "phase-08" => Ok(Self::P08),
            "9" | "09" | "phase-09" => Ok(Self::P09),
            "10" | "phase-10" => Ok(Self::P10),
            "11" | "phase-11" => Ok(Self::P11),
            "11-b" | "phase-11-b" => Ok(Self::P11B),
            "12" | "phase-12" => Ok(Self::P12),
            "13" | "phase-13" => Ok(Self::P13),
            "14" | "phase-14" => Ok(Self::P14),
            "15" | "phase-15" => Ok(Self::P15),
            "16" | "phase-16" => Ok(Self::P16),
            "17" | "phase-17" => Ok(Self::P17),
            "18" | "phase-18" => Ok(Self::P18),
            _ => Err(format!("unknown phase `{value}`; expected 00..18 or 11-b")),
        }
    }

    pub(crate) const fn number(self) -> &'static str {
        match self {
            Self::P00 => "00",
            Self::P01 => "01",
            Self::P02 => "02",
            Self::P03 => "03",
            Self::P04 => "04",
            Self::P05 => "05",
            Self::P06 => "06",
            Self::P07 => "07",
            Self::P08 => "08",
            Self::P09 => "09",
            Self::P10 => "10",
            Self::P11 => "11",
            Self::P11B => "11-b",
            Self::P12 => "12",
            Self::P13 => "13",
            Self::P14 => "14",
            Self::P15 => "15",
            Self::P16 => "16",
            Self::P17 => "17",
            Self::P18 => "18",
        }
    }
}
