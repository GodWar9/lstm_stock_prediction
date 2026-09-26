//! Sector mapping and sector classifications for portfolio risk management.
//!
//! Provides `SectorMap` which assigns instruments to economic sectors (GICS-style),
//! enabling sector-neutral portfolio optimization and sector risk factor analysis.

use quant_instruments::InstrumentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Economic sector classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sector {
    Technology,
    Financials,
    Healthcare,
    ConsumerDiscretionary,
    ConsumerStaples,
    Energy,
    Industrials,
    Materials,
    Utilities,
    RealEstate,
    CommunicationServices,
    Custom(String),
}

impl Sector {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Technology => "Technology",
            Self::Financials => "Financials",
            Self::Healthcare => "Healthcare",
            Self::ConsumerDiscretionary => "ConsumerDiscretionary",
            Self::ConsumerStaples => "ConsumerStaples",
            Self::Energy => "Energy",
            Self::Industrials => "Industrials",
            Self::Materials => "Materials",
            Self::Utilities => "Utilities",
            Self::RealEstate => "RealEstate",
            Self::CommunicationServices => "CommunicationServices",
            Self::Custom(s) => s.as_str(),
        }
    }
}

impl From<&str> for Sector {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "technology" | "tech" => Self::Technology,
            "financials" | "fin" => Self::Financials,
            "healthcare" | "health" => Self::Healthcare,
            "consumerdiscretionary" | "discretionary" => Self::ConsumerDiscretionary,
            "consumerstaples" | "staples" => Self::ConsumerStaples,
            "energy" => Self::Energy,
            "industrials" => Self::Industrials,
            "materials" => Self::Materials,
            "utilities" => Self::Utilities,
            "realestate" | "reit" => Self::RealEstate,
            "communicationservices" | "communication" | "telecom" => Self::CommunicationServices,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// Mapping of instruments to their respective sectors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectorMap {
    assignments: HashMap<InstrumentId, Sector>,
}

impl SectorMap {
    /// Creates an empty sector map.
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
        }
    }

    /// Assign an instrument to a sector.
    pub fn assign(&mut self, instrument: InstrumentId, sector: Sector) {
        self.assignments.insert(instrument, sector);
    }

    /// Get the sector for a given instrument, or default to Custom("Unassigned").
    pub fn get_sector(&self, instrument: &InstrumentId) -> Option<&Sector> {
        self.assignments.get(instrument)
    }

    /// Retrieve all distinct sectors in this mapping.
    pub fn all_sectors(&self) -> Vec<Sector> {
        let mut sectors: Vec<Sector> = self.assignments.values().cloned().collect();
        sectors.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        sectors.dedup();
        sectors
    }

    /// Find all instruments belonging to a given sector.
    pub fn instruments_in_sector(&self, sector: &Sector) -> Vec<InstrumentId> {
        self.assignments
            .iter()
            .filter(|(_, s)| *s == sector)
            .map(|(inst, _)| *inst)
            .collect()
    }

    /// Number of assigned instruments.
    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    /// True if no instruments are assigned.
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sector_classification() {
        let mut map = SectorMap::new();
        let aapl = InstrumentId(1);
        let msft = InstrumentId(2);
        let jpm = InstrumentId(3);

        map.assign(aapl, Sector::Technology);
        map.assign(msft, Sector::Technology);
        map.assign(jpm, Sector::Financials);

        assert_eq!(map.get_sector(&aapl), Some(&Sector::Technology));
        assert_eq!(map.get_sector(&jpm), Some(&Sector::Financials));

        let tech_insts = map.instruments_in_sector(&Sector::Technology);
        assert_eq!(tech_insts.len(), 2);
        assert!(tech_insts.contains(&aapl));
        assert!(tech_insts.contains(&msft));

        assert_eq!(map.all_sectors().len(), 2);
    }

    #[test]
    fn test_sector_from_str() {
        assert_eq!(Sector::from("tech"), Sector::Technology);
        assert_eq!(Sector::from("financials"), Sector::Financials);
        assert_eq!(Sector::from("crypto"), Sector::Custom("crypto".to_string()));
    }
}
