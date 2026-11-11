use excel_workbench::{engine::Engine, model::*};
use std::{path::PathBuf, time::{Duration, Instant}};

#[test]
fn failed_file_does_not_publish_or_recover_a_partial_import() {
    let temp = tempfile::TempDir::new().unwrap();
    let good = temp.path().join("good.csv");
    let bad = temp.path().join("bad.csv");
    std::fs::write(&good, "编号,金额\n001,12.5\n").unwrap();
    std::fs::write(&bad, (0..2049).map(|i| format!("field{i}")).collect::<Vec<_>>().join(",")).unwrap();
    let root = temp.path().join("data");
    let engine = Engine::open(&root).unwrap();
    let report = engine.import_files(vec![bad.to_string_lossy().into_owned(), good.to_string_lossy().into_owned()]).unwrap();
    assert!(report[0].error.is_some());
    assert!(report[1].error.is_none());
    assert_eq!(engine.snapshot().unwrap().files.len(), 1);
    drop(engine);
    let reopened = Engine::open(&root).unwrap();
    let workspace = reopened.snapshot().unwrap();
    assert_eq!(workspace.files.len(), 1);
    assert_eq!(workspace.files[0].name, "good.csv");
    let page = reopened.preview_sheet(&workspace.files[0].sheets[0].id, 1, 1, None).unwrap();
    assert_eq!(page.rows[0], vec![Cell::Text("001".into()), Cell::Text("12.5".into())]);
}

#[test]
#[ignore]
fn crash_import_child() {
    let Some(root) = std::env::var_os("GRIDFLOW_CRASH_IMPORT_ROOT") else { return; };
    let root = PathBuf::from(root);
    let engine = Engine::open(&root.join("data")).unwrap();
    let inputs = (0..300).map(|file| root.join(format!("source-{file:03}.csv")).to_string_lossy().into_owned()).collect();
    let worker = engine.clone();
    std::thread::spawn(move || worker.import_files(inputs).unwrap());
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let progress = engine.import_progress().unwrap();
        if progress.done >= 3 {
            assert!(progress.done < 300);
            std::fs::write(root.join("completed.txt"), progress.done.to_string()).unwrap();
            // Deliberately exit only this isolated child before batch completion and normal destructors.
            std::process::exit(0);
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn committed_imports_recover_before_the_batch_workspace_checkpoint() {
    let temp = tempfile::TempDir::new().unwrap();
    for file in 0..300 {
        let mut text = (0..20).map(|column| format!("field{column}")).collect::<Vec<_>>().join(",");
        text.push('\n');
        for row in 0..100 {
            text.push_str(&(0..20).map(|column| format!("{file:03}-{row:03}-{column:03}")).collect::<Vec<_>>().join(","));
            text.push('\n');
        }
        std::fs::write(temp.path().join(format!("source-{file:03}.csv")), text).unwrap();
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_import_child", "--nocapture"])
        .env("GRIDFLOW_CRASH_IMPORT_ROOT", temp.path())
        .status().unwrap();
    assert!(status.success());
    let completed: usize = std::fs::read_to_string(temp.path().join("completed.txt")).unwrap().parse().unwrap();
    let root = temp.path().join("data");
    let committed = {
        let connection = duckdb::Connection::open(root.join("workspace.duckdb")).unwrap();
        let saved: String = connection.query_row("SELECT value FROM meta WHERE key='workspace'", [], |row| row.get(0)).unwrap();
        let workspace: Workspace = serde_json::from_str(&saved).unwrap();
        assert!(workspace.files.is_empty(), "the complete workspace must be checkpointed at batch completion, not rewritten for every imported file");
        connection.query_row("SELECT count(*) FROM duckdb_tables() WHERE starts_with(table_name,'r_')", [], |row| row.get::<_, usize>(0)).unwrap()
    };
    assert!(committed >= completed && committed < 300);
    let engine = Engine::open(&root).unwrap();
    let workspace = engine.snapshot().unwrap();
    assert_eq!(workspace.files.len(), committed);
    assert!(workspace.files.iter().all(|file| file.sheets.len() == 1 && file.sheets[0].rows == 101 && file.sheets[0].width == 20));
    let mut rules = workspace.rules;
    rules.merge_mode = Some("single".into());
    rules.include_source = false;
    let data = engine.process(&rules, false, &std::sync::atomic::AtomicBool::new(false), |_,_|{}).unwrap();
    assert_eq!(data.summary.output_rows, committed * 100);
    drop(engine);
    let connection = duckdb::Connection::open(root.join("workspace.duckdb")).unwrap();
    let pending: usize = connection.query_row("SELECT count(*) FROM meta WHERE starts_with(key,'pending_import:')", [], |row| row.get(0)).unwrap();
    assert_eq!(pending, 0);
}
