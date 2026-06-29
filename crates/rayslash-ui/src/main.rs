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

    let results_model = Rc::new(VecModel::from(results));
    ui.set_results(results_model.into());

    ui.on_close_requested({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.hide().expect("failed to hide rayslash window");
            }
        }
    });

    ui.run()
}
