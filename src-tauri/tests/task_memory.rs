use excel_workbench::{engine::Engine, model::*};
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn finished_task(engine: &Engine, task_id: &str) -> Task {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let task = engine.tasks().unwrap().into_iter().find(|t| t.id == task_id).unwrap();
        if matches!(task.state, TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled) {
            return task;
        }
        assert!(Instant::now() < deadline, "task timed out: {:?}", task.state);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn batch_merge_with_saved_schemes_completes_and_preserves_task_states() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("data");
    let mut paths = Vec::new();
    for file in 0..300 {
        let path = temp.path().join(format!("synthetic-{file:03}.csv"));
        let mut text = (0..20).map(|column| format!("field{column}")).collect::<Vec<_>>().join(",");
        text.push('\n');
        for row in 0..100 {
            text.push_str(&(0..20).map(|column| format!("{file:03}-{row:03}-{column:03}")).collect::<Vec<_>>().join(","));
            text.push('\n');
        }
        std::fs::write(&path, text).unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let engine = Engine::open(&root).unwrap();
    let imported = engine.import_files(paths).unwrap();
    assert_eq!(imported.len(), 300);
    assert!(imported.iter().all(|item| item.error.is_none()));
    let mut rules = engine.snapshot().unwrap().rules;
    rules.merge_mode = Some("single".into());
    rules.include_source = false;
    // Progress must not repeatedly rewrite this populated workspace while the merge transaction is open.
    for index in 0..20 {
        engine.save_scheme(format!("saved-scheme-{index}"), rules.clone()).unwrap();
    }
    let task = engine.submit(rules.clone()).unwrap();
    let completed = finished_task(&engine, &task.id);
    assert_eq!(completed.state, TaskState::Succeeded, "{:?}", completed.error);
    assert_eq!(completed.progress, 100);
    let dataset_id = completed.dataset_id.unwrap();
    let dataset = engine.snapshot().unwrap().datasets.into_iter().find(|d| d.id == dataset_id).unwrap();
    assert_eq!(dataset.summary.input_rows, 30_000);
    assert_eq!(dataset.summary.output_rows, 30_000);
    assert_eq!(dataset.columns.len(), 20);
    let page = engine.query(DatasetQuery { dataset_id: dataset_id.clone(), offset: 29_999, limit: 1, ..Default::default() }).unwrap();
    assert_eq!(page.total, 30_000);
    assert_eq!(page.rows[0][19], Cell::Text("299-099-019".into()));

    let cancelled = engine.submit(rules).unwrap();
    engine.cancel(&cancelled.id).unwrap();
    assert_eq!(finished_task(&engine, &cancelled.id).state, TaskState::Cancelled);
    drop(engine);

    let reopened = Engine::open(&root).unwrap();
    let workspace = reopened.snapshot().unwrap();
    assert_eq!(workspace.datasets.len(), 1);
    assert_eq!(workspace.schemes.len(), 20);
    assert_eq!(workspace.tasks.iter().find(|t| t.id == task.id).unwrap().state, TaskState::Succeeded);
    assert_eq!(workspace.tasks.iter().find(|t| t.id == cancelled.id).unwrap().state, TaskState::Cancelled);
    let page = reopened.query(DatasetQuery { dataset_id, limit: 1, ..Default::default() }).unwrap();
    assert_eq!(page.total, 30_000);
    assert_eq!(page.rows[0][0], Cell::Text("000-000-000".into()));
}
