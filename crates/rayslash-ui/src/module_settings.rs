use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use rayslash_core::{
    app_state, apps, config, modules, projects, providers::ProviderExecutionHint, ranking, search,
};
use semver::Version;
use slint::{ComponentHandle, VecModel};

use crate::{
    AppWindow, ModuleItem, ResultItem,
    result_items::IconImageCache,
    runtime_state::{
        ResultRefreshContext, ResultSelection, SearchResultSet, effective_search_query,
        refresh_result_view, refresh_settings_dependent_ui, search_result_set,
    },
};

pub(crate) struct RuntimeModules {
    pub config: modules::ModulesConfig,
    pub writes_blocked: bool,
}

pub(crate) fn load_runtime_modules(
    legacy_providers: &config::ProviderConfig,
    main_config_loaded_successfully: bool,
    migrate_legacy: bool,
) -> RuntimeModules {
    if !main_config_loaded_successfully {
        return RuntimeModules {
            config: modules::ModulesConfig::from_legacy_provider_config(legacy_providers),
            writes_blocked: true,
        };
    }

    match modules::load_or_create_modules_config_with_migration(legacy_providers, migrate_legacy) {
        Ok(outcome) => RuntimeModules {
            config: outcome.into_config(),
            writes_blocked: false,
        },
        Err(error) => {
            eprintln!("{error}; using legacy provider settings and disabling module writes");
            RuntimeModules {
                config: modules::ModulesConfig::from_legacy_provider_config(legacy_providers),
                writes_blocked: true,
            }
        }
    }
}

pub(crate) fn module_items(
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
) -> Vec<ModuleItem> {
    let installed = modules::load_installed_modules().unwrap_or_default();
    let mut items = catalog
        .iter()
        .map(|module| {
            let installed_module = installed.modules.get(&module.id);
            let latest = latest_compatible_version(module);
            let update_available = installed_module
                .zip(latest)
                .is_some_and(|(installed, latest)| latest.version > installed.version);
            let version = installed_module
                .map(|installed| installed.version.to_string())
                .or_else(|| latest.map(|latest| latest.version.to_string()))
                .unwrap_or_default();
            let (icon_kind, icon_text) = module_icon(&module.id);
            ModuleItem {
                id: module.id.clone().into(),
                name: module.name.clone().into(),
                description: module.description.clone().into(),
                author: module.author.clone().into(),
                version: version.into(),
                enabled: installed_module.is_some()
                    && config
                        .is_enabled(&module.id)
                        .unwrap_or(installed_module.is_some_and(|m| m.enabled)),
                installed: installed_module.is_some(),
                official: module.official,
                stars: module.github_stars.to_string().into(),
                action: if update_available {
                    "Update".into()
                } else if installed_module.is_some() {
                    "Remove".into()
                } else {
                    "Install".into()
                },
                update_available,
                icon_kind: icon_kind.into(),
                icon_text: icon_text.into(),
            }
        })
        .collect::<Vec<_>>();
    for descriptor in modules::official_module_descriptors() {
        if items.iter().any(|item| item.id.as_str() == descriptor.id) {
            continue;
        }
        let installed_module = installed.modules.get(descriptor.id);
        let (icon_kind, icon_text) = module_icon(descriptor.id);
        items.push(ModuleItem {
            id: descriptor.id.into(),
            name: descriptor.name.into(),
            description: descriptor.description.into(),
            author: descriptor.author.into(),
            version: installed_module
                .map(|module| module.version.to_string())
                .unwrap_or_default()
                .into(),
            enabled: installed_module.is_some()
                && config.is_enabled(descriptor.id).unwrap_or(false),
            installed: installed_module.is_some(),
            official: true,
            stars: "".into(),
            action: if installed_module.is_some() {
                "Remove"
            } else {
                "Install"
            }
            .into(),
            update_available: false,
            icon_kind: icon_kind.into(),
            icon_text: icon_text.into(),
        });
    }
    items.sort_by(|left, right| {
        right
            .official
            .cmp(&left.official)
            .then_with(|| left.name.cmp(&right.name))
    });
    items
}

fn latest_compatible_version(
    module: &modules::RegistryModule,
) -> Option<&modules::RegistryVersion> {
    let api = Version::new(1, 0, 0);
    module
        .versions
        .iter()
        .filter(|version| !version.yanked && version.api_version.matches(&api))
        .max_by(|left, right| left.version.cmp(&right.version))
}

fn module_icon(module_id: &str) -> (&'static str, &'static str) {
    match module_id {
        modules::CALCULATOR_MODULE_ID => ("calculator", ""),
        modules::UNITS_MODULE_ID => ("text", "U"),
        modules::CURRENCY_MODULE_ID => ("text", "$"),
        modules::TIME_MODULE_ID => ("time", ""),
        modules::WEB_SEARCH_MODULE_ID => ("search", ""),
        modules::TIMERS_MODULE_ID => ("stopwatch", ""),
        modules::ALIASES_MODULE_ID => ("link", ""),
        _ => ("placeholder", ""),
    }
}

pub(crate) fn refresh_module_items(
    model: &Rc<VecModel<ModuleItem>>,
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
) {
    model.set_vec(module_items(config, catalog));
}

pub(crate) struct ModuleSettingsCallbackContext {
    pub module_state: Rc<RefCell<modules::ModulesConfig>>,
    pub module_catalog: Rc<RefCell<Vec<modules::RegistryModule>>>,
    pub module_model: Rc<VecModel<ModuleItem>>,
    pub module_writes_blocked: bool,
    pub config_state: Rc<RefCell<config::Config>>,
    pub app_install_state: Rc<RefCell<app_state::AppInstallState>>,
    pub ranking_state: Rc<RefCell<ranking::RankingState>>,
    pub projects: Rc<RefCell<Vec<projects::Project>>>,
    pub apps: Rc<RefCell<Vec<apps::DesktopApp>>>,
    pub current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: Rc<VecModel<ResultItem>>,
    pub icon_cache: Rc<RefCell<IconImageCache>>,
    pub socket_path: PathBuf,
    pub remote_search_generation: Arc<AtomicU64>,
    pub remote_result_tx: mpsc::Sender<(u64, String, SearchResultSet)>,
    pub profile: bool,
}

pub(crate) fn register_module_settings_callback(
    ui: &AppWindow,
    context: ModuleSettingsCallbackContext,
) {
    let ModuleSettingsCallbackContext {
        module_state,
        module_catalog,
        module_model,
        module_writes_blocked,
        config_state,
        app_install_state,
        ranking_state,
        projects,
        apps,
        current_results,
        results_model,
        icon_cache,
        socket_path,
        remote_search_generation,
        remote_result_tx,
        profile,
    } = context;

    ui.on_settings_module_action_requested({
        let weak = ui.as_weak();
        let module_state = module_state.clone();
        let module_catalog = module_catalog.clone();
        let module_model = module_model.clone();
        move |module_id, action| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if module_writes_blocked {
                ui.set_status_text(
                    "Module state is read-only until the configuration error is fixed.".into(),
                );
                return;
            }
            let module_id = module_id.as_str();
            let outcome = match action.as_str() {
                "Install" | "Update" => {
                    let catalog = module_catalog.borrow();
                    let Some(module) = catalog.iter().find(|module| module.id == module_id) else {
                        ui.set_status_text(
                            "The verified registry has no installable record for this module."
                                .into(),
                        );
                        return;
                    };
                    let Some(version) = latest_compatible_version(module) else {
                        ui.set_status_text("No compatible module version is available.".into());
                        return;
                    };
                    ui.set_status_text(format!("{} {}…", action, module.name).into());
                    modules::install_registry_version(module, version).map(|installed| {
                        let mut config = module_state.borrow().clone();
                        config.set_installed(module_id, &installed.version.to_string(), true);
                        config
                    })
                }
                "Remove" => modules::remove_installed_module(module_id, false).map(|_| {
                    let mut config = module_state.borrow().clone();
                    config.remove(module_id);
                    config
                }),
                _ => {
                    ui.set_status_text(format!("Unknown module action: {action}").into());
                    return;
                }
            };
            match outcome {
                Ok(config) => {
                    if let Err(error) = modules::save_modules_config(&config) {
                        ui.set_status_text(
                            format!("Module changed, but its setting could not be saved: {error}")
                                .into(),
                        );
                    } else {
                        *module_state.borrow_mut() = config;
                        refresh_module_items(
                            &module_model,
                            &module_state.borrow(),
                            &module_catalog.borrow(),
                        );
                        let message = if action.as_str() == "Remove" {
                            "Module code removed; its settings and data were kept".to_owned()
                        } else {
                            format!("Module {} completed", action.to_ascii_lowercase())
                        };
                        ui.set_status_text(message.into());
                    }
                }
                Err(error) => ui.set_status_text(
                    format!("Could not {} module: {error}", action.to_ascii_lowercase()).into(),
                ),
            }
        }
    });

    ui.on_settings_module_toggle_requested({
        let weak = ui.as_weak();
        move |module_id, enabled| {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            if module_writes_blocked {
                refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                ui.set_status_text(
                    "Could not save module settings: fix config.toml or modules.toml and restart rayslash."
                        .into(),
                );
                return;
            }

            let module_id = module_id.as_str();
            let module_name = module_catalog
                .borrow()
                .iter()
                .find(|module| module.id == module_id)
                .map(|module| module.name.clone())
                .or_else(|| {
                    modules::official_module_descriptor(module_id)
                        .map(|module| module.name.to_owned())
                })
                .unwrap_or_else(|| module_id.to_owned());
            let mut next_modules = module_state.borrow().clone();
            let changed = match next_modules.set_installed_enabled(module_id, enabled) {
                Ok(changed) => changed,
                Err(error) => {
                    refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                    ui.set_status_text(format!("Could not update module: {error}").into());
                    return;
                }
            };
            if !changed {
                return;
            }

            if let Err(error) = modules::save_modules_config(&next_modules) {
                eprintln!("{error}");
                refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                ui.set_status_text(format!("Could not save module setting: {error}").into());
                return;
            }

            *module_state.borrow_mut() = next_modules.clone();
            refresh_module_items(&module_model, &next_modules, &module_catalog.borrow());

            let compatibility_config = {
                let mut next_config = config_state.borrow().clone();
                next_modules.apply_to_provider_config(&mut next_config.providers);
                *config_state.borrow_mut() = next_config.clone();
                next_config
            };

            let compatibility_error =
                config::save_config_with_backup(&compatibility_config).err();
            if let Some(error) = compatibility_error.as_ref() {
                eprintln!(
                    "module state was saved, but config.toml compatibility mirror failed: {error}"
                );
            }

            let query = ui.get_query_text();
            let effective_query = effective_search_query(
                query.as_str(),
                ui.get_active_search_keyword().as_str(),
            );
            let needs_remote_lookup = matches!(
                rayslash_core::search::query_execution_hint(
                    &effective_query,
                    &config_state.borrow().providers,
                ),
                ProviderExecutionHint::DebouncedNetwork { .. }
            );

            if needs_remote_lookup {
                let generation = remote_search_generation.fetch_add(1, Ordering::Relaxed) + 1;
                let expected_generation = remote_search_generation.clone();
                let config = config_state.borrow().clone();
                let ranking = ranking_state.borrow().clone();
                let app_install = app_install_state.borrow().clone();
                let projects_snapshot = projects.borrow().clone();
                let apps_snapshot = apps.borrow().clone();
                let query = effective_query.clone();
                let remote_result_tx = remote_result_tx.clone();
                thread::spawn(move || {
                    let result_set = search_result_set(
                        &config,
                        &ranking,
                        &app_install,
                        &projects_snapshot,
                        &apps_snapshot,
                        &query,
                    );
                    if expected_generation.load(Ordering::Relaxed) == generation {
                        let _ = remote_result_tx.send((generation, query, result_set));
                    }
                });
            } else {
                remote_search_generation.fetch_add(1, Ordering::Relaxed);
                refresh_result_view(
                    &ui,
                    ResultRefreshContext {
                        config: &config_state.borrow(),
                        ranking_state: &ranking_state.borrow(),
                        app_state: &app_install_state.borrow(),
                        projects: &projects.borrow(),
                        apps: &apps.borrow(),
                        current_results: &current_results,
                        results_model: &results_model,
                        icon_cache: &icon_cache,
                        profile,
                    },
                    effective_query.as_str(),
                    ResultSelection::QueryDefault,
                );
            }
            refresh_settings_dependent_ui(
                &ui,
                &config_state.borrow(),
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );

            let state_label = if enabled { "enabled" } else { "disabled" };
            if compatibility_error.is_some() {
                ui.set_status_text(
                    format!(
                        "{} {state_label}; config.toml compatibility mirror failed.",
                        module_name
                    )
                    .into(),
                );
            } else {
                ui.set_status_text(format!("{module_name} {state_label}.").into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use rayslash_core::config::ProviderConfig;
    use rayslash_core::modules::{CALCULATOR_MODULE_ID, OFFICIAL_AUTHOR};

    use super::*;

    #[test]
    fn module_items_reflect_official_descriptors_and_enabled_state() {
        let mut config = modules::ModulesConfig::default();
        config
            .disable(CALCULATOR_MODULE_ID)
            .expect("disable calculator");

        let items = module_items(&config, &[]);
        assert_eq!(items.len(), modules::official_module_descriptors().len());
        assert!(items.iter().all(|item| item.official));
        assert!(
            items
                .iter()
                .all(|item| item.author.as_str() == OFFICIAL_AUTHOR)
        );

        let calculator = items
            .iter()
            .find(|item| item.id.as_str() == CALCULATOR_MODULE_ID)
            .expect("calculator item");
        assert_eq!(calculator.name.as_str(), "Calculator");
        assert!(!calculator.enabled);
        assert!(!calculator.description.is_empty());
    }

    #[test]
    fn failed_main_config_uses_legacy_module_state_and_blocks_writes() {
        let legacy = ProviderConfig {
            calculator: false,
            utility_actions: false,
            ..ProviderConfig::default()
        };

        let runtime = load_runtime_modules(&legacy, false, true);

        assert!(runtime.writes_blocked);
        assert_eq!(runtime.config.is_enabled(CALCULATOR_MODULE_ID), Some(false));
        assert_eq!(
            runtime
                .config
                .is_enabled(rayslash_core::modules::TIMERS_MODULE_ID),
            Some(false)
        );
    }
}
