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
) -> RuntimeModules {
    if !main_config_loaded_successfully {
        return RuntimeModules {
            config: modules::ModulesConfig::from_legacy_provider_config(legacy_providers),
            writes_blocked: true,
        };
    }

    match modules::load_or_create_modules_config(legacy_providers) {
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

pub(crate) fn module_items(config: &modules::ModulesConfig) -> Vec<ModuleItem> {
    modules::official_module_descriptors()
        .iter()
        .map(|descriptor| {
            let (icon_kind, icon_text) = module_icon(descriptor.id);
            ModuleItem {
                id: descriptor.id.into(),
                name: descriptor.name.into(),
                description: descriptor.description.into(),
                author: descriptor.author.into(),
                version: descriptor.version.into(),
                enabled: config.is_official_enabled(descriptor.id).unwrap_or(true),
                installed: true,
                official: true,
                icon_kind: icon_kind.into(),
                icon_text: icon_text.into(),
            }
        })
        .collect()
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
) {
    model.set_vec(module_items(config));
}

pub(crate) struct ModuleSettingsCallbackContext {
    pub module_state: Rc<RefCell<modules::ModulesConfig>>,
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

    ui.on_settings_module_toggle_requested({
        let weak = ui.as_weak();
        move |module_id, enabled| {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            if module_writes_blocked {
                refresh_module_items(&module_model, &module_state.borrow());
                ui.set_status_text(
                    "Could not save module settings: fix config.toml or modules.toml and restart rayslash."
                        .into(),
                );
                return;
            }

            let module_id = module_id.as_str();
            let Some(descriptor) = modules::official_module_descriptor(module_id) else {
                refresh_module_items(&module_model, &module_state.borrow());
                ui.set_status_text(format!("Unknown module: {module_id}").into());
                return;
            };

            let mut next_modules = module_state.borrow().clone();
            let changed = match next_modules.set_enabled(module_id, enabled) {
                Ok(changed) => changed,
                Err(error) => {
                    refresh_module_items(&module_model, &module_state.borrow());
                    ui.set_status_text(format!("Could not update module: {error}").into());
                    return;
                }
            };
            if !changed {
                return;
            }

            if let Err(error) = modules::save_modules_config(&next_modules) {
                eprintln!("{error}");
                refresh_module_items(&module_model, &module_state.borrow());
                ui.set_status_text(format!("Could not save module setting: {error}").into());
                return;
            }

            *module_state.borrow_mut() = next_modules.clone();
            refresh_module_items(&module_model, &next_modules);

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
                        descriptor.name
                    )
                    .into(),
                );
            } else {
                ui.set_status_text(format!("{} {state_label}.", descriptor.name).into());
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

        let items = module_items(&config);
        assert_eq!(items.len(), modules::official_module_descriptors().len());
        assert!(items.iter().all(|item| item.official && item.installed));
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
        assert!(!calculator.version.is_empty());
    }

    #[test]
    fn failed_main_config_uses_legacy_module_state_and_blocks_writes() {
        let legacy = ProviderConfig {
            calculator: false,
            utility_actions: false,
            ..ProviderConfig::default()
        };

        let runtime = load_runtime_modules(&legacy, false);

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
