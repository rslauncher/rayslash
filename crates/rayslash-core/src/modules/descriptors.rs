use std::{collections::BTreeSet, fmt};

pub const OFFICIAL_AUTHOR: &str = "rayslash";

pub const ALIASES_MODULE_ID: &str = "rayslash.aliases";
pub const CALCULATOR_MODULE_ID: &str = "rayslash.calculator";
pub const CURRENCY_MODULE_ID: &str = "rayslash.currency";
pub const TIME_MODULE_ID: &str = "rayslash.time";
pub const TIMERS_MODULE_ID: &str = "rayslash.timers";
pub const UNITS_MODULE_ID: &str = "rayslash.units";
pub const WEB_SEARCH_MODULE_ID: &str = "rayslash.web-search";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleSource {
    BuiltIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleDescriptor {
    pub id: &'static str,
    pub provider_id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub author: &'static str,
    pub version: &'static str,
    pub source: ModuleSource,
}

const OFFICIAL_MODULES: [ModuleDescriptor; 7] = [
    ModuleDescriptor {
        id: CALCULATOR_MODULE_ID,
        provider_id: CALCULATOR_MODULE_ID,
        name: "Calculator",
        description: "Calculate expressions and linear equations.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: UNITS_MODULE_ID,
        provider_id: UNITS_MODULE_ID,
        name: "Units",
        description: "Convert common units locally.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: CURRENCY_MODULE_ID,
        provider_id: CURRENCY_MODULE_ID,
        name: "Currency",
        description: "Convert currencies with cached live rates.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: TIME_MODULE_ID,
        provider_id: TIME_MODULE_ID,
        name: "Time",
        description: "Check local time for places.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: WEB_SEARCH_MODULE_ID,
        provider_id: WEB_SEARCH_MODULE_ID,
        name: "Web Search",
        description: "Search the web with keyword triggers.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: TIMERS_MODULE_ID,
        provider_id: TIMERS_MODULE_ID,
        name: "Timers",
        description: "Schedule timers, reminders, and power actions.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
    ModuleDescriptor {
        id: ALIASES_MODULE_ID,
        provider_id: ALIASES_MODULE_ID,
        name: "Aliases",
        description: "Open quick links, files, folders, and explicit commands.",
        author: OFFICIAL_AUTHOR,
        version: env!("CARGO_PKG_VERSION"),
        source: ModuleSource::BuiltIn,
    },
];

pub fn official_module_descriptors() -> &'static [ModuleDescriptor] {
    &OFFICIAL_MODULES
}

pub fn official_module_descriptor(id: &str) -> Option<&'static ModuleDescriptor> {
    OFFICIAL_MODULES
        .iter()
        .find(|descriptor| descriptor.id == id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorValidationError {
    EmptyModuleId { index: usize },
    EmptyProviderId { module_id: String },
    DuplicateModuleId { id: String },
    DuplicateProviderId { id: String },
}

impl fmt::Display for DescriptorValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyModuleId { index } => {
                write!(f, "module descriptor at index {index} has an empty ID")
            }
            Self::EmptyProviderId { module_id } => {
                write!(f, "module {module_id} has an empty provider ID")
            }
            Self::DuplicateModuleId { id } => write!(f, "duplicate module ID: {id}"),
            Self::DuplicateProviderId { id } => write!(f, "duplicate provider ID: {id}"),
        }
    }
}

impl std::error::Error for DescriptorValidationError {}

pub fn validate_descriptors(
    descriptors: &[ModuleDescriptor],
) -> Result<(), DescriptorValidationError> {
    let mut module_ids = BTreeSet::new();
    let mut provider_ids = BTreeSet::new();

    for (index, descriptor) in descriptors.iter().enumerate() {
        let module_id = descriptor.id.trim();
        if module_id.is_empty() {
            return Err(DescriptorValidationError::EmptyModuleId { index });
        }
        if !module_ids.insert(module_id) {
            return Err(DescriptorValidationError::DuplicateModuleId {
                id: module_id.to_owned(),
            });
        }

        let provider_id = descriptor.provider_id.trim();
        if provider_id.is_empty() {
            return Err(DescriptorValidationError::EmptyProviderId {
                module_id: module_id.to_owned(),
            });
        }
        if !provider_ids.insert(provider_id) {
            return Err(DescriptorValidationError::DuplicateProviderId {
                id: provider_id.to_owned(),
            });
        }
    }

    Ok(())
}
