use std::rc::Rc;

use rayslash_core::search;
use slint::VecModel;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let results = search::placeholder_results()
        .into_iter()
        .map(|result| ResultItem {
            title: result.title.into(),
            subtitle: result.subtitle.into(),
        })
        .collect::<Vec<_>>();
    let result_titles = results
        .iter()
        .map(|result| result.title.to_string())
        .collect::<Vec<_>>();

    ui.set_result_count(results.len() as i32);
    let results_model = Rc::new(VecModel::from(results));
    ui.set_results(results_model.into());
    ui.invoke_focus_search();

    ui.on_close_requested({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.hide().expect("failed to hide rayslash window");
            }
        }
    });

    ui.on_activate_placeholder({
        let weak = ui.as_weak();
        move |index| {
            let title = result_titles
                .get(index as usize)
                .cloned()
                .unwrap_or_else(|| "Unknown placeholder".to_owned());

            println!("placeholder activation: {title}");

            if let Some(ui) = weak.upgrade() {
                ui.set_status_text(format!("Preview only: {title}").into());
            }
        }
    });

    ui.run()
}
