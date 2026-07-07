use std::{ffi::OsString, hint::black_box, path::PathBuf, time::Instant};

use rayslash_core::{
    actions::CommandSpec, apps::DesktopApp, config::ProviderConfig, projects::Project,
    ranking::RankingState, search,
};

#[test]
#[ignore = "diagnostic probe; run with --ignored --nocapture when investigating search latency"]
fn mixed_search_performance_probe() {
    let apps = (0..4_000).map(synthetic_app).collect::<Vec<_>>();
    let projects = (0..1_000).map(synthetic_project).collect::<Vec<_>>();
    let providers = ProviderConfig::default();
    let ranking = RankingState::default();
    let queries = ["", "app 39", "editor", "project 42", "999 * 42"];
    let repetitions = 40;

    for query in queries {
        let started = Instant::now();
        let mut total_results = 0usize;

        for _ in 0..repetitions {
            let results = search::mixed_results_with_ranking(
                &projects,
                &apps,
                &[],
                query,
                &providers,
                Some(&ranking),
            );
            total_results += results.len();
            black_box(results);
        }

        let elapsed = started.elapsed();
        println!(
            "query={query:?} repetitions={repetitions} total_results={total_results} elapsed={elapsed:.2?} avg={:.2?}",
            elapsed / repetitions
        );
    }
}

fn synthetic_app(index: usize) -> DesktopApp {
    DesktopApp {
        id: format!("dev.rayslash.fixture.App{index}.desktop"),
        name: format!("Fixture App {index}"),
        localized_names: vec![format!("Localized Fixture App {index}")],
        generic_name: Some(if index.is_multiple_of(3) {
            "Text Editor".to_owned()
        } else {
            "Application".to_owned()
        }),
        comment: Some(format!(
            "Synthetic app used for search performance probe {index}"
        )),
        exec: "true".to_owned(),
        icon: None,
        mime_types: Vec::new(),
        categories: vec!["Utility".to_owned()],
        keywords: vec![
            "fixture".to_owned(),
            "performance".to_owned(),
            format!("group{}", index % 100),
        ],
        actions: Vec::new(),
        dbus_activatable: false,
        icon_path: None,
        command: CommandSpec {
            program: OsString::from("true"),
            args: Vec::new(),
        },
        desktop_file: PathBuf::from(format!("/tmp/rayslash-fixture-{index}.desktop")),
    }
}

fn synthetic_project(index: usize) -> Project {
    Project {
        name: format!("Fixture Project {index}"),
        path: PathBuf::from(format!("/tmp/rayslash-fixture-project-{index}")),
    }
}
