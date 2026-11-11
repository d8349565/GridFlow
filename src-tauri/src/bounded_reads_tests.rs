use super::Engine;
use crate::model::*;
use std::{path::{Path, PathBuf}, sync::{Arc, atomic::AtomicBool}, time::{Duration, Instant}};
use tempfile::TempDir;

const ROWS: usize = 6007;
const WIDTH: usize = 40;

fn header(column: usize) -> String {
    match column { 1 => "id".into(), 2 => "amount".into(), 3 => "marker".into(), _ => format!("unused{column}") }
}

fn blank(row: usize) -> bool { row % 997 == 0 || row % 2048 == 2047 }

fn field(row: usize, column: usize) -> String {
    match column {
        1 => if blank(row) { String::new() } else { format!("{row:06}") },
        2 => if blank(row) { String::new() } else { (row % 97).to_string() },
        3 => if blank(row) && row % 1994 == 0 { "keep".into() } else { String::new() },
        _ => format!("synthetic-{row:06}-{column:02}-{}", "x".repeat(360)),
    }
}

fn measured<T>(stage: &str, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let start = Instant::now();
    let result = operation();
    eprintln!("READ_TIMING {}", serde_json::json!({
        "stage": stage, "ms": start.elapsed().as_secs_f64() * 1000.,
        "ok": result.is_ok(), "error": result.as_ref().err().map(|e| &e.message)
    }));
    result
}

fn prepare(temp: &TempDir, stage: &str) -> Arc<Engine> {
    // An optional isolated fixture path lets before/after runs reuse the identical physical CSV.
    let input = std::env::var_os("GRIDFLOW_SYNTHETIC_WIDE_CSV").map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("synthetic-wide.csv"));
    if !input.exists() {
        let mut writer = csv::Writer::from_path(&input).unwrap();
        writer.write_record((0..WIDTH).map(header)).unwrap();
        for row in 0..ROWS { writer.write_record((0..WIDTH).map(|column| field(row, column))).unwrap(); }
        writer.flush().unwrap();
    }
    let engine = Engine::open(&temp.path().join("data")).unwrap();
    let imported = measured(stage, || engine.import_files(vec![input.to_string_lossy().into_owned()])).unwrap();
    assert_eq!(imported.len(), 1);
    assert!(imported[0].error.is_none(), "{:?}", imported[0].error);
    assert_eq!(imported[0].file.as_ref().unwrap().sheets[0].rows, ROWS + 1);
    engine
}

fn rules(engine: &Engine) -> RuleSet {
    let mut rules = engine.snapshot().unwrap().rules;
    rules.merge_mode = Some("single".into());
    rules.include_source = false;
    rules
}

fn publish(engine: &Engine, rules: RuleSet) -> Result<Dataset> {
    let task = engine.submit(rules)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let task = engine.tasks()?.into_iter().find(|t| t.id == task.id).unwrap();
        match task.state {
            TaskState::Succeeded => return Ok(engine.snapshot()?.datasets.into_iter()
                .find(|d| Some(&d.id) == task.dataset_id.as_ref()).unwrap()),
            TaskState::Failed | TaskState::Cancelled => return Err(AppError::new("TASK", task.error.unwrap_or(task.phase))),
            _ => {}
        }
        assert!(Instant::now() < deadline, "task timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn small_budget(engine: &Engine) {
    let connection = engine.conn().unwrap();
    connection.execute_batch("CHECKPOINT; SET memory_limit='64MiB'").unwrap();
    let limit: String = connection.query_row("SELECT current_setting('memory_limit')", [], |row| row.get(0)).unwrap();
    eprintln!("READ_BUDGET {limit}");
}

#[test]
fn merge_under_small_budget_preserves_range_skip_empty_rows_and_sample_limit() {
    let temp = TempDir::new().unwrap();
    let engine = prepare(&temp, "range_import");
    let mut rules = rules(&engine);
    rules.sources[0].header_range = Some(SheetRange { start_row: 1, end_row: 1, start_column: 2, end_column: 4 });
    rules.columns = vec![
        ColumnRule { key: "id".into(), name: "id".into(), data_type: ColumnType::Text, hidden: false },
        ColumnRule { key: "amount".into(), name: "amount".into(), data_type: ColumnType::Number, hidden: false },
        ColumnRule { key: "marker".into(), name: "marker".into(), data_type: ColumnType::Skip, hidden: false },
    ];
    rules.remove_empty_rows = true;
    small_budget(&engine);
    let dataset = measured("range_merge", || publish(&engine, rules.clone())).unwrap();
    let expected = (0..ROWS).filter(|row| !blank(*row) || !field(*row, 3).is_empty()).collect::<Vec<_>>();
    assert_eq!(dataset.summary.input_rows, ROWS);
    assert_eq!(dataset.summary.output_rows, expected.len());
    assert_eq!(dataset.summary.empty_removed, ROWS - expected.len());
    for (page, rows) in expected.chunks(1000).enumerate() {
        let actual = engine.query(DatasetQuery { dataset_id: dataset.id.clone(), offset: page * 1000, limit: 1000, ..Default::default() }).unwrap();
        assert_eq!(actual.rows.len(), rows.len());
        for (cells, row) in actual.rows.iter().zip(rows) {
            let wanted = if blank(*row) { vec![Cell::Empty, Cell::Empty] }
                else { vec![Cell::Text(field(*row, 1)), Cell::Number((*row % 97) as f64)] };
            assert_eq!(*cells, wanted, "source data row {row}");
        }
    }
    let sample = engine.process(&rules, true, &AtomicBool::new(false), |_, _| {}).unwrap();
    assert_eq!(sample.summary.input_rows, 200);
    assert_eq!(sample.summary.output_rows, (0..200).filter(|row| !blank(*row) || !field(*row, 3).is_empty()).count());

    rules.empty_columns = vec!["id".into()];
    let explicit = measured("explicit_empty_merge", || publish(&engine, rules)).unwrap();
    assert_eq!(explicit.summary.output_rows, (0..ROWS).filter(|row| !blank(*row)).count());
}

fn verify_excel(path: &Path, expected: &[usize], sheets: usize) {
    use calamine::{Data, Reader};
    let mut book = calamine::open_workbook_auto(path).unwrap();
    let names = book.sheet_names();
    assert_eq!(names.len(), sheets);
    let range = book.worksheet_range(&names[0]).unwrap();
    assert_eq!(range.height(), expected.len() + 1);
    assert_eq!(range.width(), WIDTH - 1);
    for (cells, row) in range.rows().skip(1).zip(expected) {
        assert_eq!(cells[0], Data::String(field(*row, 1)));
        assert_eq!(cells[1], Data::Float((*row % 97) as f64));
        assert_eq!(cells[WIDTH - 2], Data::String(field(*row, WIDTH - 1)));
    }
    if sheets == 2 { assert_eq!(book.worksheet_range(&names[1]).unwrap().height(), 1); }
}

#[test]
fn exports_under_small_budget_preserve_sort_filter_search_hidden_columns_and_full_length() {
    let temp = TempDir::new().unwrap();
    let engine = prepare(&temp, "export_import");
    let mut rules = rules(&engine);
    rules.columns = (0..WIDTH).map(|column| ColumnRule {
        key: header(column), name: header(column), hidden: column == 0,
        data_type: if column == 2 { ColumnType::Number } else { ColumnType::Text },
    }).collect();
    let dataset = measured("wide_merge", || publish(&engine, rules)).unwrap();
    small_budget(&engine);
    let query = DatasetQuery {
        dataset_id: dataset.id.clone(), offset: 111, limit: 7,
        sorts: vec![Sort { key: "amount".into(), descending: true }, Sort { key: "id".into(), descending: true }],
        filters: vec![Filter { key: "amount".into(), operator: "gte".into(), values: vec!["0".into()] }],
        search: "synthetic".into(),
    };
    let mut expected = (0..ROWS).filter(|row| !blank(*row)).collect::<Vec<_>>();
    expected.sort_by(|a, b| (b % 97).cmp(&(a % 97)).then_with(|| b.cmp(a)));
    let csv = temp.path().join("sorted.csv");
    measured("export_csv", || engine.export_dataset(query.clone(), csv.to_string_lossy().into_owned())).unwrap();
    let mut reader = csv::Reader::from_path(csv).unwrap();
    assert_eq!(reader.headers().unwrap().iter().collect::<Vec<_>>(), (1..WIDTH).map(header).collect::<Vec<_>>());
    let mut records = reader.records();
    for row in &expected {
        let record = records.next().expect("missing exported row").unwrap();
        for column in 1..WIDTH { assert_eq!(record.get(column - 1).unwrap(), field(*row, column), "row {row}, column {column}"); }
    }
    assert!(records.next().is_none(), "unexpected trailing exported row");

    let mut small = query;
    small.filters.push(Filter { key: "amount".into(), operator: "eq".into(), values: vec!["0".into()] });
    let expected_small = expected.into_iter().filter(|row| row % 97 == 0).collect::<Vec<_>>();
    let xlsx = temp.path().join("sorted.xlsx");
    measured("export_xlsx", || engine.export_dataset(small.clone(), xlsx.to_string_lossy().into_owned())).unwrap();
    verify_excel(&xlsx, &expected_small, 1);
    let mut empty = small.clone();
    empty.search = "no-synthetic-row-can-match-this".into();
    let workbook = temp.path().join("workbook.xlsx");
    measured("export_workbook", || engine.export_workbook(vec![small, empty], workbook.to_string_lossy().into_owned())).unwrap();
    verify_excel(&workbook, &expected_small, 2);
}

#[test]
fn exported_search_matches_result_query_for_literals_nulls_and_hidden_columns() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("search.csv");
    std::fs::write(&input, "id,category,secret\n01,A,MiXeD%_$value\n02,A,SECOND\n03,B,\n04,B,MiXeD%_$value\n05,A,ordinary\n").unwrap();
    let engine = Engine::open(&temp.path().join("data")).unwrap();
    assert!(engine.import_files(vec![input.to_string_lossy().into_owned()]).unwrap()[0].error.is_none());
    let mut rules = rules(&engine);
    rules.columns = ["id", "category", "secret"].into_iter().map(|key| ColumnRule {
        key: key.into(), name: key.into(), data_type: ColumnType::Text, hidden: key == "secret",
    }).collect();
    let dataset = publish(&engine, rules).unwrap();
    for (index, search) in ["mixed%_$", "SECOND", "  ", "missing", "value", "_", "%", "$"].into_iter().enumerate() {
        let query = DatasetQuery {
            dataset_id: dataset.id.clone(), limit: 100, search: search.into(),
            sorts: vec![Sort { key: "id".into(), descending: true }],
            filters: vec![Filter { key: "category".into(), operator: "in".into(), values: vec!["A".into(), "B".into()] }],
            ..Default::default()
        };
        let expected = engine.query(query.clone()).unwrap();
        let path = temp.path().join(format!("search-{index}.csv"));
        engine.export_dataset(query, path.to_string_lossy().into_owned()).unwrap();
        let mut reader = csv::Reader::from_path(path).unwrap();
        assert_eq!(reader.headers().unwrap().iter().collect::<Vec<_>>(), vec!["id", "category"]);
        let actual = reader.records().map(|row| row.unwrap().iter().map(str::to_string).collect::<Vec<_>>()).collect::<Vec<_>>();
        assert_eq!(actual, expected.rows.iter().map(|row| row.iter().map(Cell::text).collect::<Vec<_>>()).collect::<Vec<_>>(), "search {search:?}");
    }
}
