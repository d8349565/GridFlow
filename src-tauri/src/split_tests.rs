use super::*;
use rust_xlsxwriter::{Format, Workbook};
use std::sync::Arc;
use tempfile::TempDir;

fn fixture() -> (TempDir, Arc<Engine>, SplitService, Dataset, SplitOutput) {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("synthetic.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("部门明细").unwrap();
    for (i, label) in ["编号", "部门", "金额", "负责人"].iter().enumerate() { sheet.write_string(0, i as u16, *label).unwrap(); }
    let categories = ["销售部", "销售部", "采购部", "", "A/B", "A:B", "sales", "SALES", "未填写", "001", "1"];
    for (i, value) in categories.iter().enumerate() {
        sheet.write_number_with_format(i as u32 + 1, 0, i as f64 + 1., &Format::new().set_num_format("000")).unwrap();
        if !value.is_empty() { sheet.write_string(i as u32 + 1, 1, *value).unwrap(); }
        sheet.write_number(i as u32 + 1, 2, (i + 1) as f64 * 10.).unwrap();
        sheet.write_string(i as u32 + 1, 3, if i == 0 { "甲" } else { "乙" }).unwrap();
    }
    book.add_worksheet().set_name("其他表").unwrap().write_string(0, 0, "其他").unwrap();
    book.save(&path).unwrap();
    let core = Engine::open(&temp.path().join("workspace")).unwrap();
    let service = SplitService::new(&core).unwrap();
    let before = serde_json::to_value(core.snapshot().unwrap()).unwrap();
    let file = service.import_file(&core, path.to_str().unwrap()).unwrap();
    assert_eq!(before, serde_json::to_value(core.snapshot().unwrap()).unwrap(), "independent import must not alter summary files or rules");
    assert_eq!(file.sheets.len(), 2);
    let raw = service.source_page(&core, &file.sheets[0].id, 0, 100).unwrap();
    assert_eq!(raw.page.rows[1][0], Cell::Text("001".into()));
    let dataset = service.use_sheet(&core, &file.sheets[0].id, SheetRange { start_row: 1, end_row: 1, start_column: 1, end_column: 4 }).unwrap();
    let output = SplitOutput { directory: temp.path().to_string_lossy().into(), new_folder: true, folder_name: "拆分结果".into() };
    (temp, core, service, dataset, output)
}
fn request(dataset: &Dataset) -> SplitRequest {
    SplitRequest { query: DatasetQuery { dataset_id: dataset.id.clone(), offset: 3, limit: 1, sorts: vec![Sort { key: dataset.columns[2].key.clone(), descending: true }], ..Default::default() },
        field: dataset.columns[1].key.clone(), columns: vec![dataset.columns[2].key.clone(), dataset.columns[0].key.clone()],
        naming: vec![SplitNamePart::Text { text: "费用".into() }, SplitNamePart::Value, SplitNamePart::Date], separator: "_".into(),
        format: "xlsx".into(), source_name: "synthetic".into(), sheet_name: "部门明细".into(), date: "20261115".into() }
}

#[test]
fn imported_workbook_exports_every_group_with_reviewed_columns_sorting_and_names() {
    let (_temp, core, service, dataset, output) = fixture();
    let req = request(&dataset);
    let plan = service.prepare(&core, req, vec![], output).unwrap();
    assert_eq!(plan.total_rows, 11, "query pagination must not limit splitting");
    assert_eq!(plan.groups.len(), 10);
    assert_eq!(plan.groups.iter().find(|g| g.value == "销售部").unwrap().rows, 2);
    assert_eq!(plan.groups.iter().filter(|g| g.kind == 0 || g.value == "未填写").count(), 2);
    let names = plan.groups.iter().map(|g| g.filename.to_lowercase()).collect::<HashSet<_>>();
    assert_eq!(names.len(), plan.groups.len(), "sanitized / case-insensitive names must remain distinct");
    let sales = plan.groups.iter().find(|g| g.value == "销售部").unwrap();
    let page = service.page(&core, &plan.id, Some(&sales.id), 0, 100, vec![]).unwrap();
    assert_eq!(page.columns[0].name, "金额");
    assert_eq!(page.rows, vec![vec![Cell::Number(20.), Cell::Text("002".into())], vec![Cell::Number(10.), Cell::Text("001".into())]]);
    let ascending = service.page(&core, &plan.id, Some(&sales.id), 0, 1, vec![Sort { key: dataset.columns[2].key.clone(), descending: false }]).unwrap();
    assert_eq!(ascending.rows[0][0], Cell::Number(10.));
    let done = service.execute(&core, &plan.id).unwrap();
    assert!(done.groups.iter().all(|g| g.state == "saved"), "{done:?}");
    assert!(service.allows_path(Path::new(done.directory.as_ref().unwrap())));
    let mut count = 0;
    for group in &done.groups {
        let path = Path::new(group.path.as_ref().unwrap());
        assert_eq!(path.file_name().unwrap().to_string_lossy(), group.filename);
        let file = reader::read(path).unwrap();
        let rows = &file.sheets[0].rows;
        assert_eq!(rows[0], vec![Cell::Text("金额".into()), Cell::Text("编号".into())]);
        assert_eq!(rows.len() - 1, group.rows);
        if group.value == "销售部" { assert_eq!(rows[1..], page.rows); }
        count += rows.len() - 1;
    }
    assert_eq!(count, 11);
    assert_eq!(core.snapshot().unwrap().files.len(), 0);
}

#[test]
fn filters_ambiguous_metadata_custom_names_and_stale_preview_are_checked() {
    let (_temp, core, service, dataset, output) = fixture();
    let mut req = request(&dataset);
    req.naming = vec![SplitNamePart::Value, SplitNamePart::Column { key: dataset.columns[3].key.clone() }];
    let plan = service.prepare(&core, req.clone(), vec![], output.clone()).unwrap();
    let sales = plan.groups.iter().find(|g| g.value == "销售部").unwrap();
    assert!(sales.error.as_ref().unwrap().contains("多个值"));
    assert_eq!(service.execute(&core, &plan.id).unwrap_err().code, "NAME");
    let override_name = SplitEdit { id: sales.id.clone(), selected: true, name: Some("CON.txt".into()) };
    let renamed = service.prepare(&core, req.clone(), vec![override_name], output.clone()).unwrap();
    assert!(renamed.groups.iter().find(|g| g.value == "销售部").unwrap().filename.starts_with("_CON.txt"));
    assert_eq!(service.execute(&core, &plan.id).unwrap_err().code, "STALE");
    req.query.filters.push(Filter { key: dataset.columns[2].key.clone(), operator: "gte".into(), values: vec!["20".into()] });
    req.query.search = "销售".into();
    let filtered = service.prepare(&core, req.clone(), vec![], output.clone()).unwrap();
    assert_eq!(filtered.total_rows, 1);
    assert!(filtered.groups[0].error.is_none(), "metadata uniqueness is scoped to the filtered group");
    let invalid = service.prepare(&core, req.clone(), vec![SplitEdit { id: filtered.groups[0].id.clone(), selected: true, name: Some(" ".into()) }], output.clone()).unwrap();
    assert!(invalid.groups[0].error.is_some());
    req.query.search = "nonexistent".into();
    let empty = service.prepare(&core, req, vec![], output).unwrap();
    assert_eq!(empty.total_rows, 0); assert!(empty.groups.is_empty());
}

#[test]
fn export_never_overwrites_files_created_before_or_after_preview_and_retry_skips_successes() {
    let (temp, core, service, dataset, mut output) = fixture();
    output.new_folder = false;
    let req = request(&dataset);
    let existing = temp.path().join("费用_销售部_20261115.xlsx");
    std::fs::write(&existing, b"existing sentinel").unwrap();
    let plan = service.prepare(&core, req, vec![], output).unwrap();
    let sales = plan.groups.iter().find(|g| g.value == "销售部").unwrap();
    assert_ne!(sales.filename, existing.file_name().unwrap().to_string_lossy());
    let raced_group = &plan.groups[0];
    let raced = temp.path().join(&raced_group.filename);
    std::fs::write(&raced, b"new sentinel").unwrap();
    let first = service.execute(&core, &plan.id).unwrap();
    assert_eq!(first.groups.iter().filter(|g| g.state == "failed").count(), 1);
    assert_eq!(std::fs::read(&existing).unwrap(), b"existing sentinel");
    assert_eq!(std::fs::read(&raced).unwrap(), b"new sentinel");
    let successful = first.groups.iter().filter_map(|g| g.path.as_ref()).map(|path| (path.clone(), std::fs::metadata(path).unwrap().modified().unwrap())).collect::<Vec<_>>();
    // This path was created by this test, never a user file.
    std::fs::remove_file(&raced).unwrap();
    let retried = service.execute(&core, &plan.id).unwrap();
    assert!(retried.groups.iter().all(|g| g.state == "saved"));
    assert_eq!(service.progress().total, 1);
    for (path, modified) in successful { assert_eq!(std::fs::metadata(path).unwrap().modified().unwrap(), modified); }
    assert_eq!(std::fs::read(&existing).unwrap(), b"existing sentinel");
}

#[test]
fn existing_summary_supports_more_than_one_distinct_page_and_selected_csv_exports() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("synthetic-summary.csv");
    let mut writer = csv::Writer::from_path(&input).unwrap();
    writer.write_record(["部门", "编号"]).unwrap();
    for i in 0..205 { writer.write_record([format!("分类{i:03}"), format!("{i:05}")]).unwrap(); }
    writer.flush().unwrap();
    let core = Engine::open(&temp.path().join("workspace")).unwrap();
    let dataset = core.import_results(vec![input.to_string_lossy().into()]).unwrap().remove(0);
    let service = SplitService::new(&core).unwrap();
    let req = SplitRequest { query: DatasetQuery { dataset_id: dataset.id.clone(), limit: 1, offset: 100, ..Default::default() },
        field: dataset.columns[0].key.clone(), columns: dataset.columns.iter().map(|c| c.key.clone()).collect(), naming: vec![SplitNamePart::Value], separator: "_".into(), format: "csv".into(), source_name: "总表".into(), sheet_name: "总表".into(), date: "20261115".into() };
    let output = SplitOutput { directory: temp.path().to_string_lossy().into(), new_folder: true, folder_name: "csv拆分".into() };
    let all = service.prepare(&core, req.clone(), vec![], output.clone()).unwrap();
    assert_eq!(all.groups.len(), 205);
    let edits = all.groups.iter().map(|g| SplitEdit { id: g.id.clone(), selected: g.value == "分类000" || g.value == "分类204", name: None }).collect();
    let selected = service.prepare(&core, req, edits, output).unwrap();
    assert_eq!(selected.selected_rows, 2);
    let done = service.execute(&core, &selected.id).unwrap();
    assert_eq!(done.groups.iter().filter(|g| g.state == "saved").count(), 2);
    for group in done.groups.iter().filter(|g| g.selected) {
        let bytes = std::fs::read(group.path.as_ref().unwrap()).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains(&group.value));
        assert!(text.contains(if group.value == "分类000" { "00000" } else { "00204" }));
    }
}

#[test]
fn multilevel_headers_preserve_selected_column_range_and_source_replacement_cleans_up() {
    let (temp, core, service, old_dataset, _output) = fixture();
    let path = temp.path().join("multilevel.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.merge_range(0, 1, 0, 2, "费用", &Format::new()).unwrap();
    sheet.write_string(1, 1, "部门").unwrap(); sheet.write_string(1, 2, "金额").unwrap();
    sheet.write_string(2, 1, "部门甲").unwrap(); sheet.write_number(2, 2, 12.).unwrap();
    book.save(&path).unwrap();
    let file = service.import_file(&core, path.to_str().unwrap()).unwrap();
    assert!(core.conn().unwrap().prepare(&format!("SELECT * FROM d_{}", old_dataset.id)).is_err());
    let ds = service.use_sheet(&core, &file.sheets[0].id, SheetRange { start_row: 1, end_row: 2, start_column: 2, end_column: 3 }).unwrap();
    assert_eq!(ds.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["费用 / 部门", "费用 / 金额"]);
    let req = SplitRequest { query: DatasetQuery { dataset_id: ds.id.clone(), ..Default::default() }, field: ds.columns[0].key.clone(), columns: ds.columns.iter().map(|c| c.key.clone()).collect(), naming: vec![SplitNamePart::Value], separator: "_".into(), format: "xlsx".into(), source_name: "multi".into(), sheet_name: "multi".into(), date: "20261115".into() };
    let plan = service.prepare(&core, req, vec![], SplitOutput { directory: String::new(), new_folder: true, folder_name: "output".into() }).unwrap();
    assert_eq!(plan.groups[0].value, "部门甲"); assert_eq!(plan.total_rows, 1);
    assert_eq!(service.execute(&core, &plan.id).unwrap_err().code, "DIRECTORY");
}

#[test]
fn mixed_types_whitespace_empty_cells_and_unicode_names_preserve_every_source_record() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("typed.xlsx");
    let values = vec![Cell::Number(1.), Cell::Text("1".into()), Cell::Text("001".into()), Cell::Text("1 ".into()),
        Cell::Empty, Cell::Text(String::new()), Cell::Bool(true), Cell::Text("true".into()),
        Cell::Date("2026-11-15 00:00:00".into()), Cell::Text("2026-11-15 00:00:00".into())];
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.write_string(0, 0, "分类").unwrap(); sheet.write_string(0, 1, "记录号").unwrap();
    for (i, value) in values.iter().enumerate() {
        let row = i as u32 + 1;
        match value {
            Cell::Empty => {},
            Cell::Text(s) => { sheet.write_string(row, 0, s).unwrap(); },
            Cell::Number(n) => { sheet.write_number(row, 0, *n).unwrap(); },
            Cell::Bool(b) => { sheet.write_boolean(row, 0, *b).unwrap(); },
            Cell::Date(_) => { sheet.write_datetime_with_format(row, 0, &rust_xlsxwriter::ExcelDateTime::from_ymd(2026, 11, 15).unwrap(), &Format::new().set_num_format("yyyy-mm-dd")).unwrap(); },
        }
        sheet.write_string(row, 1, format!("ID-{i:03}")).unwrap();
    }
    book.save(&input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let core = Engine::open(&temp.path().join("workspace")).unwrap();
    let service = SplitService::new(&core).unwrap();
    let source = service.import_file(&core, input.to_str().unwrap()).unwrap();
    let dataset = service.use_sheet(&core, &source.sheets[0].id, SheetRange { start_row: 1, end_row: 1, start_column: 1, end_column: 2 }).unwrap();
    let req = SplitRequest { query: DatasetQuery { dataset_id: dataset.id.clone(), ..Default::default() }, field: dataset.columns[0].key.clone(),
        columns: dataset.columns.iter().map(|c| c.key.clone()).collect(), naming: vec![SplitNamePart::Value], separator: "_".into(), format: "xlsx".into(), source_name: "typed".into(), sheet_name: "typed".into(), date: "20261115".into() };
    let output = SplitOutput { directory: temp.path().to_string_lossy().into(), new_folder: true, folder_name: "typed-output".into() };
    let initial = service.prepare(&core, req.clone(), vec![], output.clone()).unwrap();
    assert_eq!(initial.groups.len(), 9, "only empty string and empty cell share a group");
    assert_eq!(initial.groups.iter().find(|g| g.kind == 0).unwrap().rows, 2);
    assert_eq!(initial.groups.iter().filter(|g| g.value == "1").count(), 2, "text 1 and numeric 1 remain separate");
    let edits = initial.groups.iter().filter_map(|g| {
        let name = if g.value == "001" { Some("CON .txt".into()) } else if g.kind == 4 { Some("边界🙂".repeat(100)) } else { None };
        name.map(|name| SplitEdit { id: g.id.clone(), selected: true, name: Some(name) })
    }).collect();
    let preview = service.prepare(&core, req, edits, output).unwrap();
    assert_eq!(preview.groups.iter().map(|g| g.filename.to_lowercase()).collect::<HashSet<_>>().len(), 9);
    let done = service.execute(&core, &preview.id).unwrap();
    assert!(done.groups.iter().all(|g| g.state == "saved"), "{done:?}");
    let mut observed = HashSet::new();
    for group in done.groups {
        let read = reader::read(Path::new(group.path.as_ref().unwrap())).unwrap();
        assert_eq!(read.sheets[0].rows.len() - 1, group.rows);
        for cells in read.sheets[0].rows.iter().skip(1) {
            let id = cells[1].text();
            assert!(observed.insert(id.clone()), "duplicate exported record {id}");
            let i: usize = id.strip_prefix("ID-").unwrap().parse().unwrap();
            let expected = &values[i];
            if expected.text().is_empty() { assert!(cells[0].text().is_empty()); }
            else { assert_eq!(&cells[0], expected); }
        }
    }
    assert_eq!(observed.len(), values.len());
    assert_eq!(std::fs::read(&input).unwrap(), original, "the imported original is untouched");
}

#[test]
fn cancellation_during_a_real_export_keeps_completed_files_and_retry_finishes_only_the_remainder() {
    use std::time::{Duration, Instant};
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("cancel.csv");
    let mut writer = csv::Writer::from_path(&input).unwrap();
    writer.write_record(["分类", "编号", "内容"]).unwrap();
    writer.write_record(["先完成", "00000", "保留已完成文件"]).unwrap();
    for i in 1..=8192 { writer.write_record(["随后停止".to_string(), format!("{i:05}"), format!("自造记录-{i}-{}", "x".repeat(512))]).unwrap(); }
    writer.flush().unwrap();
    let core = Engine::open(&temp.path().join("workspace")).unwrap();
    let service = SplitService::new(&core).unwrap();
    let source = service.import_file(&core, input.to_str().unwrap()).unwrap();
    let dataset = service.use_sheet(&core, &source.sheets[0].id, SheetRange { start_row: 1, end_row: 1, start_column: 1, end_column: 3 }).unwrap();
    let req = SplitRequest { query: DatasetQuery { dataset_id: dataset.id.clone(), ..Default::default() }, field: dataset.columns[0].key.clone(), columns: dataset.columns.iter().map(|c| c.key.clone()).collect(), naming: vec![SplitNamePart::Value], separator: "_".into(), format: "xlsx".into(), source_name: "cancel".into(), sheet_name: "cancel".into(), date: "20261115".into() };
    let plan = service.prepare(&core, req, vec![], SplitOutput { directory: temp.path().to_string_lossy().into(), new_folder: true, folder_name: "cancel-output".into() }).unwrap();
    let stopped = std::thread::scope(|scope| {
        let export = scope.spawn(|| service.execute(&core, &plan.id));
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let progress = service.progress();
            if progress.busy && progress.done == 1 && progress.current == "随后停止.xlsx" { service.cancel(); break; }
            assert!(Instant::now() < deadline, "did not reach the second export: {progress:?}");
            std::thread::sleep(Duration::from_millis(1));
        }
        export.join().unwrap().unwrap()
    });
    assert_eq!(stopped.groups[0].state, "saved");
    assert_eq!(stopped.groups[1].state, "cancelled");
    let first_path = Path::new(stopped.groups[0].path.as_ref().unwrap());
    let first_bytes = std::fs::read(first_path).unwrap();
    let directory = Path::new(stopped.directory.as_ref().unwrap());
    assert!(!directory.join("随后停止.xlsx").exists());
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1, "no partial file or temporary file is published");
    let retried = service.execute(&core, &plan.id).unwrap();
    assert!(retried.groups.iter().all(|g| g.state == "saved"));
    assert_eq!(service.progress().total, 1);
    assert_eq!(std::fs::read(first_path).unwrap(), first_bytes);
    let read = reader::read(Path::new(retried.groups[1].path.as_ref().unwrap())).unwrap();
    assert_eq!(read.sheets[0].rows.len(), 8193);
    assert_eq!(read.sheets[0].rows[8192][1].text(), "08192");
}

#[test]
fn failed_import_invalid_header_and_missing_output_do_not_destroy_the_current_source() {
    let (temp, core, service, dataset, output) = fixture();
    let bad = temp.path().join("corrupt.xlsx");
    std::fs::write(&bad, b"not an Excel workbook").unwrap();
    assert!(service.import_file(&core, bad.to_str().unwrap()).is_err());
    assert!(service.use_sheet(&core, "unknown-sheet", SheetRange { start_row: 1, end_row: 1, start_column: 1, end_column: 4 }).is_err());
    let mut invalid_output = output.clone();
    invalid_output.directory = temp.path().join("missing-parent").to_string_lossy().into();
    assert_eq!(service.prepare(&core, request(&dataset), vec![], invalid_output).unwrap_err().code, "DIRECTORY");
    assert!(!temp.path().join("missing-parent").exists());
    let recovered = service.prepare(&core, request(&dataset), vec![], output).unwrap();
    assert_eq!(recovered.total_rows, 11);
    assert!(service.execute(&core, &recovered.id).unwrap().groups.iter().all(|g| g.state == "saved"));
}

#[test]
fn completed_merge_task_can_be_filtered_and_split_without_losing_or_duplicating_rows() {
    use std::time::{Duration, Instant};
    let temp = TempDir::new().unwrap();
    let first = temp.path().join("first.csv");
    let second = temp.path().join("second.csv");
    std::fs::write(&first, "编号,部门,金额,审批\n0001,销售部,10,通过\n0002,采购部,20,待审\n0003,销售部,30,通过\n").unwrap();
    std::fs::write(&second, "编号,部门,金额,审批\n0004,采购部,40,通过\n0005,销售部,50,通过\n0006,销售部,60,待审\n").unwrap();
    let core = Engine::open(&temp.path().join("workspace")).unwrap();
    core.import_files(vec![first.to_string_lossy().into(), second.to_string_lossy().into()]).unwrap();
    let mut rules = core.snapshot().unwrap().rules;
    rules.merge_mode = Some("single".into()); rules.output_sheets.clear();
    for source in &mut rules.sources { source.enabled = true; source.header_row = 1; }
    let task = core.submit(rules).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let dataset = loop {
        let state = core.snapshot().unwrap();
        let current = state.tasks.iter().find(|t| t.id == task.id).unwrap();
        assert!(!matches!(current.state, TaskState::Failed | TaskState::Cancelled), "{current:?}");
        if current.state == TaskState::Succeeded {
            break state.datasets.iter().find(|d| Some(&d.id) == current.dataset_id.as_ref()).unwrap().clone();
        }
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(dataset.summary.output_rows, 6);
    let state_before = core.snapshot().unwrap();
    let service = SplitService::new(&core).unwrap();
    let request = SplitRequest { query: DatasetQuery { dataset_id: dataset.id.clone(), offset: 2, limit: 1, filters: vec![Filter { key: "审批".into(), operator: "eq".into(), values: vec!["通过".into()] }], sorts: vec![Sort { key: "金额".into(), descending: true }], search: String::new() },
        field: "部门".into(), columns: vec!["编号".into(), "部门".into(), "金额".into()], naming: vec![SplitNamePart::Value, SplitNamePart::Text { text: "已通过".into() }], separator: "_".into(), format: "xlsx".into(), source_name: dataset.name.clone(), sheet_name: dataset.name.clone(), date: "20261115".into() };
    let plan = service.prepare(&core, request, vec![], SplitOutput { directory: temp.path().to_string_lossy().into(), new_folder: true, folder_name: "merged-output".into() }).unwrap();
    assert_eq!(plan.total_rows, 4); assert_eq!(plan.groups.len(), 2);
    let done = service.execute(&core, &plan.id).unwrap();
    let mut observed = Vec::new();
    for group in &done.groups {
        assert_eq!(group.state, "saved");
        let output = reader::read(Path::new(group.path.as_ref().unwrap())).unwrap();
        let rows = &output.sheets[0].rows;
        assert_eq!(rows[0].iter().map(Cell::text).collect::<Vec<_>>(), ["编号", "部门", "金额"]);
        for row in rows.iter().skip(1) { assert_eq!(row[1].text(), group.value); observed.push(row[0].text()); }
        if group.value == "销售部" { assert_eq!(rows.iter().skip(1).map(|r| r[0].text()).collect::<Vec<_>>(), ["0005", "0003", "0001"]); }
    }
    observed.sort(); assert_eq!(observed, ["0001", "0003", "0004", "0005"]);
    let after = core.snapshot().unwrap();
    assert_eq!(serde_json::to_value(&after.rules).unwrap(), serde_json::to_value(&state_before.rules).unwrap());
    assert_eq!(after.files.len(), 2); assert_eq!(after.datasets.len(), 1);
}
