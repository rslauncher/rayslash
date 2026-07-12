mod fixtures;

use std::{collections::BTreeSet, fs};

use fixtures::TempDir;
use rayslash_core::{
    config::ProviderConfig,
    modules::{
        ALIASES_MODULE_ID, CALCULATOR_MODULE_ID, CURRENCY_MODULE_ID, DescriptorValidationError,
        LoadModulesConfigError, MODULES_CONFIG_VERSION, ModuleDescriptor, ModuleSource,
        ModulesConfig, ModulesConfigLoadOutcome, OFFICIAL_AUTHOR, TIME_MODULE_ID, TIMERS_MODULE_ID,
        UNITS_MODULE_ID, WEB_SEARCH_MODULE_ID, load_modules_config_from_path,
        load_or_create_modules_config_from_path,
        load_or_create_modules_config_from_path_with_migration, modules_config_file,
        official_module_descriptors, save_modules_config_to_path, validate_descriptors,
    },
};

#[test]
fn official_descriptors_are_unique_virtual_modules() {
    let descriptors = official_module_descriptors();
    validate_descriptors(descriptors).expect("official descriptors should be valid");

    let ids = descriptors
        .iter()
        .map(|descriptor| descriptor.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 7);
    assert_eq!(
        ids,
        BTreeSet::from([
            ALIASES_MODULE_ID,
            CALCULATOR_MODULE_ID,
            CURRENCY_MODULE_ID,
            TIME_MODULE_ID,
            TIMERS_MODULE_ID,
            UNITS_MODULE_ID,
            WEB_SEARCH_MODULE_ID,
        ])
    );
    assert!(descriptors.iter().all(|descriptor| {
        descriptor.author == OFFICIAL_AUTHOR
            && descriptor.source == ModuleSource::BuiltIn
            && descriptor.id == descriptor.provider_id
    }));
}

#[test]
fn descriptor_validation_rejects_duplicate_module_and_provider_ids() {
    const FIRST: ModuleDescriptor = ModuleDescriptor {
        id: "rayslash.first",
        provider_id: "rayslash.first",
        name: "First",
        description: "First provider.",
        author: OFFICIAL_AUTHOR,
        version: "1.0.0",
        source: ModuleSource::BuiltIn,
    };
    const DUPLICATE_MODULE: ModuleDescriptor = ModuleDescriptor {
        id: "rayslash.first",
        provider_id: "rayslash.second",
        name: "Duplicate",
        description: "Duplicate module.",
        author: OFFICIAL_AUTHOR,
        version: "1.0.0",
        source: ModuleSource::BuiltIn,
    };
    const DUPLICATE_PROVIDER: ModuleDescriptor = ModuleDescriptor {
        id: "rayslash.second",
        provider_id: "rayslash.first",
        name: "Duplicate",
        description: "Duplicate provider.",
        author: OFFICIAL_AUTHOR,
        version: "1.0.0",
        source: ModuleSource::BuiltIn,
    };

    assert!(matches!(
        validate_descriptors(&[FIRST, DUPLICATE_MODULE]),
        Err(DescriptorValidationError::DuplicateModuleId { .. })
    ));
    assert!(matches!(
        validate_descriptors(&[FIRST, DUPLICATE_PROVIDER]),
        Err(DescriptorValidationError::DuplicateProviderId { .. })
    ));
}

#[test]
fn missing_modules_config_is_seeded_from_legacy_provider_settings() {
    let dir = TempDir::new("rayslash-modules-missing");
    let legacy = ProviderConfig {
        apps: false,
        folders: false,
        calculator: false,
        aliases: true,
        web_search: false,
        unit_conversion: true,
        currency_conversion: false,
        time_lookup: true,
        utility_actions: false,
    };

    let config = load_modules_config_from_path(&dir.join("missing.toml"), &legacy)
        .expect("missing module config should migrate legacy provider settings");

    assert_eq!(config.version, MODULES_CONFIG_VERSION);
    assert_eq!(config.is_enabled(CALCULATOR_MODULE_ID), Some(false));
    assert_eq!(config.is_enabled(ALIASES_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(WEB_SEARCH_MODULE_ID), Some(false));
    assert_eq!(config.is_enabled(UNITS_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(CURRENCY_MODULE_ID), Some(false));
    assert_eq!(config.is_enabled(TIME_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(TIMERS_MODULE_ID), Some(false));
    assert!(!config.modules.contains_key("rayslash.apps"));
    assert!(!config.modules.contains_key("rayslash.folders"));
}

#[test]
fn existing_module_entries_override_legacy_and_missing_entries_are_seeded() {
    let dir = TempDir::new("rayslash-modules-partial");
    let path = dir
        .write(
            "modules.toml",
            r#"
version = 1

[modules."rayslash.calculator"]
enabled = true
"#,
        )
        .expect("write partial module config");
    let legacy = ProviderConfig {
        calculator: false,
        aliases: false,
        ..ProviderConfig::default()
    };

    let config = load_modules_config_from_path(&path, &legacy).expect("load module config");

    assert_eq!(config.is_enabled(CALCULATOR_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(ALIASES_MODULE_ID), Some(false));
}

#[test]
fn enable_disable_rejects_unknown_modules_without_removing_them() {
    let mut config = ModulesConfig::default();
    assert_eq!(config.disable(CALCULATOR_MODULE_ID), Ok(true));
    assert_eq!(config.disable(CALCULATOR_MODULE_ID), Ok(false));
    assert_eq!(config.enable(CALCULATOR_MODULE_ID), Ok(true));

    let unknown = "community.example.docs";
    let unknown_entry = config
        .modules
        .get(CALCULATOR_MODULE_ID)
        .expect("calculator entry")
        .clone();
    config.modules.insert(unknown.to_owned(), unknown_entry);
    assert!(config.disable(unknown).is_err());
    assert!(config.modules.contains_key(unknown));
}

#[test]
fn compatibility_helpers_preserve_apps_and_folders() {
    let legacy = ProviderConfig {
        apps: false,
        folders: true,
        calculator: false,
        aliases: true,
        web_search: false,
        unit_conversion: true,
        currency_conversion: false,
        time_lookup: true,
        utility_actions: false,
    };
    let mut config = ModulesConfig::from_legacy_provider_config(&legacy);
    config.enable(CALCULATOR_MODULE_ID).expect("enable");
    config.disable(ALIASES_MODULE_ID).expect("disable");

    let applied = config.applied_provider_config(&legacy);
    assert!(!applied.apps);
    assert!(applied.folders);
    assert!(applied.calculator);
    assert!(!applied.aliases);

    let changed_legacy = ProviderConfig {
        apps: true,
        folders: false,
        calculator: false,
        aliases: true,
        web_search: true,
        unit_conversion: false,
        currency_conversion: true,
        time_lookup: false,
        utility_actions: true,
    };
    config.mirror_from_provider_config(&changed_legacy);
    assert_eq!(config.is_enabled(CALCULATOR_MODULE_ID), Some(false));
    assert_eq!(config.is_enabled(ALIASES_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(UNITS_MODULE_ID), Some(false));
    assert_eq!(config.is_enabled(CURRENCY_MODULE_ID), Some(true));
    assert_eq!(config.is_enabled(TIMERS_MODULE_ID), Some(true));
}

#[test]
fn unknown_modules_and_fields_survive_round_trip() {
    let dir = TempDir::new("rayslash-modules-unknown");
    let source = dir
        .write(
            "source.toml",
            r#"
version = 1
future_top_level = "preserve me"

[modules."community.example.docs"]
enabled = false
version = "2.0.0"
channel = "beta"
future_permission = "docs.example.com"
"#,
        )
        .expect("write module config");
    let config = load_modules_config_from_path(&source, &ProviderConfig::default())
        .expect("load module config");
    let destination = dir.join("nested/modules.toml");

    save_modules_config_to_path(&destination, &config).expect("save module config");
    let reloaded = load_modules_config_from_path(&destination, &ProviderConfig::default())
        .expect("reload module config");

    assert_eq!(
        reloaded.extra.get("future_top_level"),
        Some(&toml::Value::String("preserve me".to_owned()))
    );
    let unknown = reloaded
        .modules
        .get("community.example.docs")
        .expect("unknown module should be preserved");
    assert!(!unknown.enabled);
    assert_eq!(unknown.version.as_deref(), Some("2.0.0"));
    assert_eq!(unknown.channel.as_deref(), Some("beta"));
    assert_eq!(
        unknown.extra.get("future_permission"),
        Some(&toml::Value::String("docs.example.com".to_owned()))
    );
}

#[test]
fn corrupt_and_unsupported_configs_return_specific_errors() {
    let dir = TempDir::new("rayslash-modules-errors");
    let corrupt = dir
        .write("corrupt.toml", "version = [")
        .expect("write corrupt config");
    assert!(matches!(
        load_modules_config_from_path(&corrupt, &ProviderConfig::default()),
        Err(LoadModulesConfigError::Parse { .. })
    ));

    let unsupported = dir
        .write("unsupported.toml", "version = 99\n")
        .expect("write unsupported config");
    assert!(matches!(
        load_modules_config_from_path(&unsupported, &ProviderConfig::default()),
        Err(LoadModulesConfigError::UnsupportedVersion { version: 99, .. })
    ));
}

#[test]
fn saved_modules_config_is_complete_and_parseable() {
    let dir = TempDir::new("rayslash-modules-save");
    let path = dir.join("nested/modules.toml");
    let mut config = ModulesConfig::default();
    config.disable(WEB_SEARCH_MODULE_ID).expect("disable web");

    save_modules_config_to_path(&path, &config).expect("save module config");
    let contents = fs::read_to_string(&path).expect("read saved config");
    assert!(contents.contains(&format!("version = {MODULES_CONFIG_VERSION}")));

    let loaded = load_modules_config_from_path(&path, &ProviderConfig::default())
        .expect("load saved config");
    assert_eq!(loaded, config);
}

#[test]
fn load_or_create_writes_migrated_config_only_on_first_run() {
    let dir = TempDir::new("rayslash-modules-create");
    let path = dir.join("nested/modules.toml");
    let legacy = ProviderConfig {
        calculator: false,
        utility_actions: false,
        ..ProviderConfig::default()
    };

    let first = load_or_create_modules_config_from_path(&path, &legacy)
        .expect("create migrated module config");
    assert!(matches!(first, ModulesConfigLoadOutcome::Created(_)));
    assert!(first.was_created());
    assert_eq!(first.config().is_enabled(CALCULATOR_MODULE_ID), Some(false));
    assert_eq!(first.config().is_enabled(TIMERS_MODULE_ID), Some(false));
    let first_contents = fs::read_to_string(&path).expect("read created config");

    let changed_legacy = ProviderConfig::default();
    let second = load_or_create_modules_config_from_path(&path, &changed_legacy)
        .expect("load existing module config");
    assert!(matches!(second, ModulesConfigLoadOutcome::Loaded(_)));
    assert!(!second.was_created());
    assert_eq!(
        second.config().is_enabled(CALCULATOR_MODULE_ID),
        Some(false)
    );
    assert_eq!(second.config().is_enabled(TIMERS_MODULE_ID), Some(false));
    assert_eq!(
        fs::read_to_string(&path).expect("reread module config"),
        first_contents
    );
}

#[test]
fn fresh_install_creates_an_empty_optional_module_config() {
    let dir = TempDir::new("rayslash-modules-fresh");
    let path = dir.join("modules.toml");
    let outcome = load_or_create_modules_config_from_path_with_migration(
        &path,
        &ProviderConfig::default(),
        false,
    )
    .expect("create fresh module config");

    assert!(outcome.was_created());
    assert!(outcome.config().modules.is_empty());
    let reloaded = load_modules_config_from_path(&path, &ProviderConfig::default())
        .expect("reload fresh module config");
    assert!(reloaded.modules.is_empty());
}

#[test]
fn modules_config_path_is_separate_from_main_config() {
    let Some(path) = modules_config_file() else {
        return;
    };
    assert!(path.ends_with("rayslash/modules.toml"));
}
