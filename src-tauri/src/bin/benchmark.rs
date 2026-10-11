use excel_workbench::{engine::Engine, model::*};
use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".cache")
        .join(format!("benchmark-{}", id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("100000x20.csv");
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&file)?);
    writeln!(
        writer,
        "编号,{}",
        (1..20)
            .map(|i| format!("数值{i}"))
            .collect::<Vec<_>>()
            .join(",")
    )?;
    for row in 0..100_000 {
        write!(writer, "{row:08}")?;
        for col in 1..20 {
            write!(writer, ",{}", row % 1000 + col)?;
        }
        writeln!(writer)?;
    }
    writer.flush()?;
    drop(writer);
    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));
    let sample_stop = stop.clone();
    let sample_peak = peak.clone();
    let pid = sysinfo::get_current_pid()?;
    let sampler = std::thread::spawn(move || {
        let mut sys = sysinfo::System::new();
        while !sample_stop.load(Ordering::Relaxed) {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            if let Some(proc) = sys.process(pid) {
                sample_peak.fetch_max(proc.memory(), Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    });
    let start = Instant::now();
    let engine = Engine::open(&directory.join("data"))?;
    let startup_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let imported = engine.import_files(vec![file.to_string_lossy().into()])?;
    if let Some(error) = &imported[0].error {
        return Err(error.clone().into());
    }
    let import_ms = start.elapsed().as_millis();
    let mut rules = engine.snapshot()?.rules;
    rules.include_source = false;
    rules.columns = (1..20)
        .map(|i| ColumnRule {
            key: format!("数值{i}"),
            name: format!("数值{i}"),
            data_type: ColumnType::Number,
            hidden: false,
        })
        .collect();
    let start = Instant::now();
    let task = engine.submit(rules)?;
    let dataset = loop {
        let ws = engine.snapshot()?;
        let task = ws.tasks.iter().find(|t| t.id == task.id).unwrap();
        if task.state == TaskState::Succeeded {
            break ws
                .datasets
                .iter()
                .find(|d| Some(&d.id) == task.dataset_id.as_ref())
                .unwrap()
                .clone();
        }
        if task.state == TaskState::Failed {
            return Err(task.error.clone().unwrap_or_default().into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let process_ms = start.elapsed().as_millis();
    let query = DatasetQuery {
        dataset_id: dataset.id.clone(),
        offset: 50000,
        limit: 100,
        ..Default::default()
    };
    let mut pages = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let page = engine.query(query.clone())?;
        assert_eq!(page.rows.len(), 100);
        pages.push(start.elapsed().as_secs_f64() * 1000.);
    }
    pages.sort_by(f64::total_cmp);
    let start = Instant::now();
    let filtered = engine.query(DatasetQuery {
        offset: 0,
        filters: vec![Filter {
            key: "数值1".into(),
            operator: "gt".into(),
            values: vec!["900".into()],
        }],
        ..query.clone()
    })?;
    let filter_ms = start.elapsed().as_secs_f64() * 1000.;
    assert_eq!(filtered.total, 10000);
    let start = Instant::now();
    engine.export_dataset(
        DatasetQuery {
            offset: 0,
            ..query.clone()
        },
        directory.join("result.xlsx").to_string_lossy().into(),
    )?;
    let export_ms = start.elapsed().as_millis();
    stop.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    let report = serde_json::json!({"rows":100000,"columns":20,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"engineStartupMs":startup_ms,"importMs":import_ms,"processMs":process_ms,"pageMedianMs":pages[5],"pageP95Ms":pages[9],"filterMs":filter_ms,"xlsxExportMs":export_ms,"peakWorkingSetMiB":peak.load(Ordering::Relaxed) as f64/1048576.,"dataDirectory":directory});
    let text = serde_json::to_string_pretty(&report)?;
    std::fs::write(directory.parent().unwrap().join("benchmark.json"), &text)?;
    println!("{text}");
    Ok(())
}
