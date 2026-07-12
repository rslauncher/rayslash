mod descriptors;
mod package;
mod registry;
mod state;

pub use descriptors::{
    ALIASES_MODULE_ID, CALCULATOR_MODULE_ID, CURRENCY_MODULE_ID, DescriptorValidationError,
    ModuleDescriptor, ModuleSource, OFFICIAL_AUTHOR, TIME_MODULE_ID, TIMERS_MODULE_ID,
    UNITS_MODULE_ID, WEB_SEARCH_MODULE_ID, official_module_descriptor, official_module_descriptors,
    validate_descriptors,
};
pub use package::{
    InstalledModule, InstalledModules, ModulePackageManifest, PackageError, PackageKind,
    PackagePermissions, install_registry_version, installed_modules_file, load_installed_modules,
    remove_installed_module,
};
pub use registry::{
    DEFAULT_REGISTRY_ROOT_URL, RAW_REGISTRY_ROOT_URL, RegistryIndex, RegistryModule,
    RegistryRefresh, RegistryRoot, RegistryVersion, ReviewStatus, load_cached_registry,
    refresh_registry, verify_registry_bytes,
};
pub use state::{
    InitializeModulesConfigError, LoadModulesConfigError, MODULES_CONFIG_VERSION,
    ModuleEntryConfig, ModulesConfig, ModulesConfigLoadOutcome, SaveModulesConfigError,
    UnknownModuleError, load_modules_config, load_modules_config_from_path,
    load_or_create_modules_config, load_or_create_modules_config_from_path, modules_config_file,
    save_modules_config, save_modules_config_to_path,
};
