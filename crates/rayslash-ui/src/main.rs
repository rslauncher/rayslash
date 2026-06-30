use std::{cell::RefCell, rc::Rc};

use rayslash_core::{config, projects, search};
use slint::VecModel;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let config = config::load_config().unwrap_or_else(|error| {
        eprintln!("{error}; using default config");
        config::Config::default()
    });
    let projects = Rc::new(projects::scan_project_roots(&config.project_roots));
    let current_results = Rc::new(RefCell::new(search::project_results(&projects, "")));
    let results_model = Rc::new(VecModel::from(to_result_items(&current_results.borrow())));

    ui.set_result_count(current_results.borrow().len() as i32);
    ui.set_results(results_model.clone().into());
    ui.invoke_focus_search();

    ui.on_close_requested({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.hide().expect("failed to hide rayslash window");
            }
        }
    });

    ui.on_query_changed({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        move |query| {
            let results = search::project_results(&projects, query.as_str());
            let count = results.len() as i32;

            results_model.set_vec(to_result_items(&results));
            *current_results.borrow_mut() = results;

            if let Some(ui) = weak.upgrade() {
                ui.set_result_count(count);
                ui.set_selected_index(0);
            }
        }
    });

    ui.on_activate_selected_result({
        let weak = ui.as_weak();
        let current_results = current_results.clone();
        move |index| {
            let result = current_results.borrow().get(index as usize).cloned();

            match result {
                Some(result) => {
                    if let Some(path) = result.project_path() {
                        println!("Selected project: {}", path.display());

                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(
                                format!("Selected project: {}", path.display()).into(),
                            );
                        }
                    } else {
                        println!("placeholder activation: {}", result.title);

                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(format!("Preview only: {}", result.title).into());
                        }
                    }
                }
                None => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("No result selected.".into());
                    }
                }
            }
        }
    });

    ui.run()
}

fn to_result_items(results: &[search::SearchResult]) -> Vec<ResultItem> {
    results
        .iter()
        .map(|result| ResultItem {
            title: result.title.clone().into(),
            subtitle: result.subtitle.clone().into(),
        })
        .collect()
}
