use excel_workbench::{compatibility, engine::Engine, model::*, reader};
use rust_xlsxwriter::Workbook;
use std::{path::Path, sync::atomic::AtomicBool};
use tempfile::TempDir;

fn csv_file(root: &Path, name: &str, contents: &str) -> String {
    let path = root.join(name);
    std::fs::write(&path, contents).unwrap();
    path.to_string_lossy().into()
}
fn rules_for(engine: &Engine) -> RuleSet {
    let mut rules = engine.snapshot().unwrap().rules;
    for source in &mut rules.sources {
        source.header_row = 1;
    }
    rules
}
fn publish(engine: &Engine, rules: RuleSet) -> Dataset {
    let task = engine.submit(rules).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let ws = engine.snapshot().unwrap();
        let task = ws.tasks.iter().find(|t| t.id == task.id).unwrap();
        if task.state == TaskState::Succeeded {
            return ws
                .datasets
                .iter()
                .find(|d| Some(&d.id) == task.dataset_id.as_ref())
                .unwrap()
                .clone();
        }
        assert!(
            !matches!(task.state, TaskState::Failed | TaskState::Cancelled),
            "{:?}",
            task
        );
        assert!(std::time::Instant::now() < deadline, "task timed out");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
fn query(id: &str) -> DatasetQuery {
    DatasetQuery {
        dataset_id: id.into(),
        limit: 100,
        ..Default::default()
    }
}
fn numeric(key: &str) -> ColumnRule {
    ColumnRule {
        key: key.into(),
        name: key.into(),
        data_type: ColumnType::Number,
        hidden: false,
    }
}

#[test]
fn multirow_header_range_styles_fixed_values_and_schemes() {
    use rust_xlsxwriter::{Format,Color};
    let temp=TempDir::new().unwrap();
    let mut paths=Vec::new();
    for (index,name) in ["上海店","北京店"].iter().enumerate() {
        let mut book=Workbook::new(); let s=book.add_worksheet();s.set_name("销售").unwrap();
        let heading=Format::new().set_font_name("Microsoft YaHei").set_font_size(16).set_bold().set_background_color(Color::RGB(0x2563eb)).set_font_color(Color::White).set_border(rust_xlsxwriter::FormatBorder::Thin);
        s.merge_range(0,0,0,4,"销售报表",&heading).unwrap();
        s.write_string(1,0,"门店名称").unwrap();s.write_string(1,1,*name).unwrap();
        s.merge_range(3,1,4,1,"门店编号",&heading).unwrap();s.merge_range(3,2,3,3,"收入",&heading).unwrap();
        s.write_string_with_format(4,2,"金额",&heading).unwrap();s.write_string_with_format(4,3,"数量",&heading).unwrap();
        s.set_column_width(2,22.).unwrap();
        s.set_row_height(0,36.).unwrap();
        s.write_string(5,0,"不要合并的说明").unwrap();s.write_string(5,1,"001").unwrap();s.write_number(5,2,1200.).unwrap();s.write_number(5,3,3.).unwrap();
        s.write_string(6,1,"002").unwrap();s.write_string(6,2,"待确认").unwrap();s.write_number(6,3,0.).unwrap();
        let p=temp.path().join(format!("销售{index}.xlsx"));book.save(&p).unwrap();paths.push(p.to_string_lossy().into_owned());
    }
    let data=temp.path().join("isolated-data");let engine=Engine::open(&data).unwrap();assert!(engine.import_files(paths).unwrap().iter().all(|i|i.error.is_none()));
    let ws=engine.snapshot().unwrap();let area=SheetRange { start_row:4,end_row:5,start_column:2,end_column:4 };
    let look=engine.preview_appearance(&ws.files[0].sheets[0].id,0,100).unwrap();
    assert!(look.merges.iter().any(|r| r.start_row==4 && r.end_row==5));
    assert!(look.styles.iter().any(|s| s.bold && s.background.eq_ignore_ascii_case("#2563eb")));
    assert!(look.styles.iter().any(|s|s.font_name=="Microsoft YaHei"&&s.font_size==16.&&s.borders.iter().any(|b|!b.is_empty())));
    assert!(look.widths[2]>140.);
    assert_eq!(look.row_heights.get(&1),Some(&48.));
    assert!(look.cells.iter().any(|c|c.row==6&&c.column==3&&c.raw_number==Some(1200.)));
    let mut rules=ws.rules;
    for source in &mut rules.sources { source.header_row=5;source.header_range=Some(area.clone()); }
    rules.include_source=false;
    rules.fixed_cells.push(FixedCellRule { id:"shop".into(),name:"门店名称".into(),sheet_name:"销售".into(),address:"B2".into() });
    rules.columns.push(numeric("收入 / 金额"));
    let cols=engine.input_columns(&rules).unwrap();
    assert_eq!(cols.iter().map(|c|c.name.as_str()).collect::<Vec<_>>(),vec!["门店编号","收入 / 金额","收入 / 数量"]);
    let suggestions=engine.suggestions(&rules).unwrap();
    assert!(suggestions.iter().find(|s| s.name=="收入 / 数量").unwrap().examples.iter().any(|s| s.before==Cell::Number(3.)));
    let preview=engine.preview_rules(rules.clone()).unwrap();assert_eq!(preview.summary.output_rows,4);
    let result=publish(&engine,rules.clone());let page=engine.query(query(&result.id)).unwrap();
    assert_eq!(page.rows[0],vec![Cell::Number(1200.),Cell::Text("001".into()),Cell::Number(3.),Cell::Text("上海店".into())]);
    assert!(page.rows.iter().any(|r|r.contains(&Cell::Text("北京店".into()))));
    let issues=engine.issues(&result.id,0).unwrap();assert_eq!(issues.len(),2);assert!(issues.iter().all(|i|i.row==7&&i.source_column==3));
    let scheme=engine.save_scheme("多层表头".into(),rules.clone()).unwrap();
    assert!(scheme.sheets.iter().all(|p|p.header_range==Some(area.clone())));
    let mut reset=rules.clone();for source in &mut reset.sources { source.header_row=1;source.header_range=None; }
    let restored=engine.apply_scheme(&scheme.id,reset).unwrap();assert!(restored.sources.iter().all(|s|s.header_row==5&&s.header_range==Some(area.clone())));
    let output=temp.path().join("result.xlsx");engine.export_dataset(query(&result.id),output.to_string_lossy().into_owned()).unwrap();
    let values=reader::read(&output).unwrap();assert!(values.sheets[0].rows[0].contains(&Cell::Text("收入 / 金额".into())));
    let imported=compatibility::convert(serde_json::to_value(scheme).unwrap()).unwrap();assert!(imported.1.iter().all(|s|s.header_range==Some(area.clone())));
    let mut invalid=rules;invalid.sources[0].header_range.as_mut().unwrap().end_column=99;assert!(engine.preview_rules(invalid).is_err());
}

#[test]
fn formatted_identifier_zeroes_survive_full_processing_and_raw_values_stay_numeric() {
    let tmp=TempDir::new().unwrap();let path=tmp.path().join("编号.xlsx");
    let mut book=Workbook::new();let s=book.add_worksheet();s.write_string(0,0,"商品编号").unwrap();
    let format=rust_xlsxwriter::Format::new().set_num_format("000000");
    for row in 1..=1101 {s.write_number_with_format(row,0,(row+23) as f64,&format).unwrap();}
    book.save(&path).unwrap();let engine=Engine::open(&tmp.path().join("data")).unwrap();
    engine.import_files(vec![path.to_string_lossy().into_owned()]).unwrap();let rules=rules_for(&engine);
    let sheet=engine.snapshot().unwrap().files[0].sheets[0].clone();
    assert_eq!(engine.preview_sheet(&sheet.id,1,1,None).unwrap().rows[0][0],Cell::Number(24.));
    assert_eq!(engine.suggestions(&rules).unwrap()[0].examples[0].after,Cell::Text("000024".into()));
    let ds=publish(&engine,rules.clone());assert_eq!(ds.summary.output_rows,1101);
    let mut q=query(&ds.id);q.offset=1100;q.limit=1;
    assert_eq!(engine.query(q).unwrap().rows[0][0],Cell::Text("001124".into()));
    let mut numeric_rules=rules;numeric_rules.columns.push(numeric("商品编号"));
    let numeric_ds=publish(&engine,numeric_rules);assert_eq!(engine.query(query(&numeric_ds.id)).unwrap().rows[0][0],Cell::Number(24.));
}
fn specimen(path: &Path, marker: &str) {
    let mut book = Workbook::new();
    let ws = book.add_worksheet();
    ws.set_name("数据").unwrap();
    ws.write_string(0, 0, marker).unwrap();
    ws.write_string(1, 0, "编号").unwrap();
    ws.write_string(1, 1, "金额").unwrap();
    ws.write_string(2, 0, "00123").unwrap();
    ws.write_string(2, 1, "￥1,234.50").unwrap();
    book.save(path).unwrap();
}

#[test]
fn conversion_identifier_errors_and_export_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    let path = csv_file(
        tmp.path(),
        "金额.csv",
        "编号,金额,日期\n00123,\"￥1,234.50\",2026/10/10\n00234,错误值,2026-10-11\n",
    );
    assert!(engine.import_files(vec![path]).unwrap()[0].error.is_none());
    let mut rules = rules_for(&engine);
    rules.include_source = false;
    rules.columns = vec![
        ColumnRule {
            key: "编号".into(),
            name: "客户编号".into(),
            data_type: ColumnType::Text,
            hidden: false,
        },
        numeric("金额"),
        ColumnRule {
            key: "日期".into(),
            name: "日期".into(),
            data_type: ColumnType::Date,
            hidden: false,
        },
    ];
    let ds = publish(&engine, rules);
    assert_eq!(ds.summary.output_rows, 2);
    assert_eq!(ds.summary.issue_count, 1);
    let page = engine.query(query(&ds.id)).unwrap();
    assert_eq!(page.rows[0][0], Cell::Text("00123".into()));
    assert_eq!(page.rows[0][1], Cell::Number(1234.5));
    assert_eq!(page.rows[1][1], Cell::Text("错误值".into()));
    let issue = engine.issues(&ds.id, 0).unwrap();
    assert_eq!(issue[0].row, 3);
    assert_eq!(issue[0].column, "金额");
    let out = tmp.path().join("导出 结果.xlsx");
    engine
        .export_dataset(query(&ds.id), out.to_string_lossy().into())
        .unwrap();
    let roundtrip = reader::read(&out).unwrap();
    assert_eq!(roundtrip.sheets[0].rows[1][0], Cell::Text("00123".into()));
    assert_eq!(roundtrip.sheets[0].rows[1][1], Cell::Number(1234.5));
    let csv = tmp.path().join("导出.csv");
    engine
        .export_dataset(query(&ds.id), csv.to_string_lossy().into())
        .unwrap();
    assert!(std::fs::read(csv).unwrap().starts_with(&[0xef, 0xbb, 0xbf]));
}
#[test]
fn same_display_source_keeps_fixed_cells_separate_after_hiding_and_rename() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("甲.xlsx");
    let b = tmp.path().join("乙.xlsx");
    specimen(&a, "甲部门");
    specimen(&b, "乙部门");
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    engine
        .import_files(vec![a.to_string_lossy().into(), b.to_string_lossy().into()])
        .unwrap();
    let mut rules = rules_for(&engine);
    for source in &mut rules.sources {
        source.source_name = "同名来源".into();
        source.header_row = 2;
    }
    rules.fixed_cells = vec![FixedCellRule {
        id: "department".into(),
        name: "部门".into(),
        sheet_name: "数据".into(),
        address: "A1".into(),
    }];
    rules.columns = vec![
        ColumnRule {
            key: "__source".into(),
            name: "来源改名".into(),
            data_type: ColumnType::Text,
            hidden: true,
        },
        numeric("金额"),
    ];
    let ds = publish(&engine, rules);
    let page = engine.query(query(&ds.id)).unwrap();
    let i = page.columns.iter().position(|c| c.name == "部门").unwrap();
    assert_eq!(page.rows[0][i], Cell::Text("甲部门".into()));
    assert_eq!(page.rows[1][i], Cell::Text("乙部门".into()));
    assert!(!page.columns.iter().any(|c| c.key == "__source"));
}
#[test]
fn weighted_average_multiple_values_zero_null_and_filter_export() {
    let tmp = TempDir::new().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    engine
        .import_files(vec![csv_file(
            tmp.path(),
            "透视.csv",
            "分组,类别,金额,数量\n甲,A,0,1\n甲,A,10,2\n甲,A,20,3\n甲,B,100,4\n乙,A,,5\n",
        )])
        .unwrap();
    let mut rules = rules_for(&engine);
    rules.include_source = false;
    rules.columns = vec![numeric("金额"), numeric("数量")];
    let ds = publish(&engine, rules);
    let spec = PivotSpec {
        query: query(&ds.id),
        row_fields: vec!["分组".into()],
        column_fields: vec!["类别".into()],
        value_fields: vec!["金额".into(), "数量".into()],
        aggregation: "avg".into(),
        show_row_totals: true,
        sort: None,
        custom_order: vec![],
    };
    let analysis = engine.aggregate(spec.clone(), None).unwrap();
    assert_eq!(analysis.total_records, 5);
    let sum = analysis
        .columns
        .iter()
        .position(|c| c.key == "total:0")
        .unwrap();
    let row = analysis
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text("甲".into()))
        .unwrap();
    assert_eq!(row[sum], Cell::Number(32.5));
    let empty = analysis
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text("乙".into()))
        .unwrap();
    assert_eq!(empty[1], Cell::Empty);
    assert_eq!(analysis.rows.last().unwrap()[sum], Cell::Number(32.5));
    let path = tmp.path().join("透视.xlsx");
    engine
        .export_analysis(spec, path.to_string_lossy().into())
        .unwrap();
    let exported = reader::read(&path).unwrap();
    let exported_row = exported.sheets[0]
        .rows
        .iter()
        .find(|r| r.first() == Some(&Cell::Text("甲".into())))
        .unwrap();
    assert_eq!(exported_row[sum], Cell::Number(32.5));
    let mut q = query(&ds.id);
    q.filters = vec![Filter {
        key: "分组".into(),
        operator: "in".into(),
        values: vec!["乙".into()],
    }];
    assert_eq!(engine.query(q.clone()).unwrap().total, 1);
    let path = tmp.path().join("筛选.csv");
    engine
        .export_dataset(q, path.to_string_lossy().into())
        .unwrap();
    assert_eq!(reader::read(&path).unwrap().sheets[0].rows.len(), 2);
}
#[test]
fn union_intersection_dedup_and_partial_preview() {
    let tmp = TempDir::new().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    engine
        .import_files(vec![
            csv_file(tmp.path(), "甲.csv", "编号,金额\n1,10\n1,20\n2,30\n"),
            csv_file(tmp.path(), "乙.csv", "编号,部门\n3,销售\n"),
        ])
        .unwrap();
    let mut rules = rules_for(&engine);
    rules.include_source = false;
    rules.remove_duplicates = true;
    rules.duplicate_columns = vec!["编号".into()];
    rules.keep_strategy = "last".into();
    let ds = publish(&engine, rules.clone());
    assert_eq!(ds.summary.output_rows, 3);
    assert_eq!(ds.summary.duplicates_removed, 1);
    assert_eq!(ds.columns.len(), 3);
    rules.merge_strategy = "inner".into();
    let ds = publish(&engine, rules.clone());
    assert_eq!(ds.columns.len(), 1);
    let preview = engine.preview_rules(rules).unwrap();
    assert!(preview.sample);
    assert_eq!(preview.summary.output_rows, 3);
}
#[test]
fn persistence_password_cancellation_and_sample_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("data");
    let engine = Engine::open(&root).unwrap();
    let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/excel");
    let paths = std::fs::read_dir(samples)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().into())
        .collect();
    let report = engine.import_files(paths).unwrap();
    assert_eq!(report.len(), 5);
    assert!(report.iter().all(|r| r.error.is_none()), "{:?}", report);
    let mut rules = rules_for(&engine);
    rules.remove_empty_rows = true;
    let ds = publish(&engine, rules.clone());
    assert!(ds.summary.output_rows > 0);
    engine
        .save_draft(rules.clone(), engine.snapshot().unwrap().active_sheet_id)
        .unwrap();
    let scheme = engine
        .save_scheme("通用方案".into(), rules.clone())
        .unwrap();
    assert!(engine.apply_scheme(&scheme.id, rules.clone()).is_ok());
    let again = engine
        .save_scheme("通用方案".into(), rules.clone())
        .unwrap();
    assert_eq!(again.id, scheme.id, "同名保存应覆盖而不是新增");
    assert_eq!(engine.snapshot().unwrap().schemes.len(), 1);
    let renamed = engine.rename_scheme(&scheme.id, "月度销售".into()).unwrap();
    assert_eq!(renamed.name, "月度销售");
    assert!(engine.rename_scheme(&scheme.id, "  ".into()).is_err());
    assert_eq!(
        engine.snapshot().unwrap().schemes[0].name,
        "月度销售"
    );
    assert_eq!(
        engine
            .process(&rules, false, &AtomicBool::new(true), |_, _| {})
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    engine.set_password("test1234", "").unwrap();
    engine.lock().unwrap();
    assert!(engine.snapshot().unwrap().locked);
    assert_eq!(engine.query(query(&ds.id)).unwrap_err().code, "LOCKED");
    assert!(engine.unlock("wrong").is_err());
    engine.unlock("test1234").unwrap();
    assert!(!engine.snapshot().unwrap().locked);
    drop(engine);
    let reopened = Engine::open(&root).unwrap();
    assert!(reopened.snapshot().unwrap().locked);
    reopened.unlock("test1234").unwrap();
    assert_eq!(reopened.snapshot().unwrap().files.len(), 5);
    assert_eq!(
        reopened.query(query(&ds.id)).unwrap().total,
        ds.summary.output_rows
    );
}
#[test]
fn legacy_coordinates_number_alias_and_safe_sql() {
    let (_,presets)=compatibility::convert(serde_json::json!({"file_configs":[{"file_id":"obsolete","sheet_name":"数据","header_row":1}],"cleaning_options":{"column_types":{"金额":"numeric"}}})).unwrap();
    assert_eq!(presets[0].header_row, 2);
    let (rules,_) = compatibility::convert(serde_json::json!({"version":"1.3","columns":[{"originalName":"金额","displayName":"收入","columnType":"number","visible":true}],"files":[{"fileId":"old","sheets":[{"name":"数据","headerRow":2,"enabled":true}]}]})).unwrap();
    assert_eq!(rules.columns[0].data_type, ColumnType::Number);
    assert_eq!(rules.columns[0].name, "收入");
    assert_eq!(reader::parse_address("AA10").unwrap(), (10, 27));
    assert!(reader::parse_address("A0").is_err());
    assert_eq!(reader::parse_number("12%"), Some(0.12));
    assert_eq!(reader::parse_number("(1,234.50)"), Some(-1234.5));
    assert_eq!(
        reader::suggest_type(&[Cell::Text("0001".into())]),
        ColumnType::Text
    );
    let tmp = TempDir::new().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    let data = csv_file(
        tmp.path(),
        "特殊.csv",
        "字段,金额\n\"x' OR 1=1 --\",10\n其他,20\n",
    );
    engine.import_files(vec![data]).unwrap();
    let ds = publish(&engine, rules_for(&engine));
    let mut q = query(&ds.id);
    q.filters.push(Filter {
        key: "字段".into(),
        operator: "in".into(),
        values: vec!["x' OR 1=1 --".into()],
    });
    assert_eq!(engine.query(q).unwrap().total, 1);
}

#[test]
fn xls_csv_encodings_headers_dates_and_readonly_numeric_analysis() {
    let xls = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/any_sheets.xls");
    let parsed = reader::read(&xls).unwrap();
    assert!(!parsed.sheets.is_empty());
    assert!(parsed.sheets.iter().any(|s| !s.rows.is_empty()));
    let formatted=Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/original-formats.xls");
    let look=excel_workbench::appearance::read(&formatted,"原格式",3,0,100).unwrap();
    assert!(look.styles.iter().any(|s|s.font_name=="SimSun"&&s.font_size==16.&&s.bold&&s.background.eq_ignore_ascii_case("#0000FF")));
    assert!(look.styles.iter().any(|s|s.number_format=="\"¥\"#,##0.00"));
    assert_eq!(look.row_heights.get(&1),Some(&48.));
    assert_eq!(look.row_heights.get(&4),Some(&0.));
    assert_eq!(look.widths[2],0.);
    assert!(look.cells.iter().any(|c|c.row==2&&c.column==2&&c.raw_number==Some(1234.5)));
    let tmp = TempDir::new().unwrap();
    let text = "姓名,金额\r\n张三,100\r\n";
    let (gbk, _, _) = encoding_rs::GBK.encode(text);
    let gbk_path = tmp.path().join("GBK.csv");
    std::fs::write(&gbk_path, gbk).unwrap();
    assert_eq!(
        reader::read(&gbk_path).unwrap().sheets[0].rows[1][0],
        Cell::Text("张三".into())
    );
    let utf16 = tmp.path().join("UTF16.csv");
    let mut bytes = vec![255, 254];
    for word in text.encode_utf16() {
        bytes.extend(word.to_le_bytes());
    }
    std::fs::write(&utf16, bytes).unwrap();
    assert_eq!(
        reader::read(&utf16).unwrap().sheets[0].rows[1][0],
        Cell::Text("张三".into())
    );
    let names = reader::header_names(
        &[
            Cell::Text("金额".into()),
            Cell::Text("金额".into()),
            Cell::Text("金额 #2".into()),
        ],
        3,
    );
    assert_eq!(
        names.iter().collect::<std::collections::HashSet<_>>().len(),
        3
    );
    assert!(reader::parse_date("02/03/2026").is_none());
    assert_eq!(
        reader::parse_date_with_format("02/03/2026", "dmy"),
        Some("2026-03-02".into())
    );
    assert_eq!(
        reader::parse_date_with_format("02/03/2026", "mdy"),
        Some("2026-02-03".into())
    );
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    let file = csv_file(
        tmp.path(),
        "不修改.csv",
        "门店,金额\n甲,￥100\n甲,￥200\n乙,￥300\n",
    );
    engine.import_files(vec![file]).unwrap();
    let suggestions = engine.suggestions(&rules_for(&engine)).unwrap();
    assert!(suggestions.iter().all(|h| !h.name.starts_with("input:")));
    let ds = publish(&engine, rules_for(&engine));
    let spec = PivotSpec {
        query: query(&ds.id),
        row_fields: vec!["门店".into()],
        column_fields: vec![],
        value_fields: vec!["金额".into()],
        aggregation: "sum".into(),
        show_row_totals: true,
        sort: None,
        custom_order: vec![],
    };
    let analysis = engine.aggregate(spec, None).unwrap();
    assert_eq!(analysis.rows.last().unwrap()[1], Cell::Number(600.));
    let page = engine.query(query(&ds.id)).unwrap();
    let amount = page.columns.iter().position(|c| c.name == "金额").unwrap();
    assert_eq!(page.rows[0][amount], Cell::Text("￥100".into()));
}
#[test]
fn fixed_cell_formula_issues_missing_sheets_and_cleanup() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("公式.xlsx");
    let mut book = Workbook::new();
    let data = book.add_worksheet();
    data.set_name("数据").unwrap();
    data.write_string(0, 0, "金额").unwrap();
    data.write_number(1, 0, 10.).unwrap();
    let meta = book.add_worksheet();
    meta.set_name("信息").unwrap();
    meta.write_formula(0, 0, "=1+2").unwrap();
    book.save(&input).unwrap();
    // Remove the formula cache in our own generated fixture, as Excel sometimes does.
    let archive = std::fs::File::open(&input).unwrap();
    let mut zip = zip::ZipArchive::new(archive).unwrap();
    let rewritten = tmp.path().join("无缓存.xlsx");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&rewritten).unwrap());
    for i in 0..zip.len() {
        let mut item = zip.by_index(i).unwrap();
        let name = item.name().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut item, &mut bytes).unwrap();
        if name == "xl/worksheets/sheet2.xml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace("<v>0</v>", "")
                .into_bytes();
        }
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut writer, &bytes).unwrap();
    }
    writer.finish().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    engine
        .import_files(vec![rewritten.to_string_lossy().into()])
        .unwrap();
    let ws = engine.snapshot().unwrap();
    let file = ws.files[0].clone();
    let mut rules = rules_for(&engine);
    rules
        .sources
        .iter_mut()
        .for_each(|s| s.enabled = s.sheet_id == file.sheets[0].id);
    rules.fixed_cells.push(FixedCellRule {
        id: "cached".into(),
        name: "固定公式".into(),
        sheet_name: "信息".into(),
        address: "A1".into(),
    });
    let ds = publish(&engine, rules.clone());
    let issue = engine.issues(&ds.id, 0).unwrap();
    assert_eq!(issue.len(), 1);
    assert_eq!(issue[0].sheet_name, "信息");
    assert_eq!(issue[0].row, 1);
    assert_eq!(issue[0].source_column, 1);
    rules.fixed_cells[0].sheet_name = "不存在".into();
    assert_eq!(
        engine
            .process(&rules, false, &AtomicBool::new(false), |_, _| {})
            .unwrap_err()
            .code,
        "MISSING_SHEET"
    );
    engine.delete_items("datasets", &[ds.id.clone()]).unwrap();
    assert_eq!(engine.query(query(&ds.id)).unwrap_err().code, "NOT_FOUND");
    engine.delete_items("files", &[file.id]).unwrap();
    assert!(engine.snapshot().unwrap().files.is_empty());
    assert!(input.exists());
    assert!(rewritten.exists());
}

#[test]
fn dataset_category_04_form_metadata_and_clean_suggestions() {
    let base = Path::new("../test_datasets");
    if !base.exists() {
        return;
    }
    // 寻找分类 04_分支机构费用申报 中的实际申报单 Excel 文件
    fn find_cat04(dir: &Path) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if p.is_dir() {
                if name.starts_with("04_") || dir.to_string_lossy().contains("04_") {
                    if let Some(found) = find_cat04(&p) {
                        return Some(found);
                    }
                }
            } else if p.extension().map_or(false, |ext| ext == "xlsx")
                && dir.to_string_lossy().contains("04_")
                && name.starts_with("费用申报_BJ01")
            {
                return Some(p);
            }
        }
        None
    }

    let Some(file_path) = find_cat04(base) else {
        return;
    };
    let tmp = TempDir::new().unwrap();
    let engine = Engine::open(&tmp.path().join("data")).unwrap();
    let imported = engine
        .import_files(vec![file_path.to_string_lossy().into()])
        .unwrap();
    assert!(imported[0].error.is_none());

    let ws = engine.snapshot().unwrap();
    let sheet = &ws.files[0].sheets[0];

    // 核心断言 1：表头行必须正确识别为第 3 行，跳过前两行带冒号的表单信息
    assert_eq!(sheet.suggested_header_row, 3);
    assert_eq!(
        sheet
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "报销单号",
            "费用发生日期",
            "费用科目",
            "费用详细事由",
            "发票张数",
            "申请报销金额",
            "实批报销金额",
            "结算支付方式"
        ]
    );

    // 核心断言 2：建议类型必须符合业务语义
    assert_eq!(
        sheet
            .columns
            .iter()
            .find(|c| c.name == "报销单号")
            .unwrap()
            .suggested_type,
        ColumnType::Text
    );
    assert_eq!(
        sheet
            .columns
            .iter()
            .find(|c| c.name == "费用发生日期")
            .unwrap()
            .suggested_type,
        ColumnType::Date
    );
    assert_eq!(
        sheet
            .columns
            .iter()
            .find(|c| c.name == "申请报销金额")
            .unwrap()
            .suggested_type,
        ColumnType::Number
    );

    // 核心断言 3：engine.suggestions 不应产出任何荒谬的格式变更建议（无张建国改日期、无王立群错误、无冒号标签）
    let rules = ws.rules.clone();
    let suggestions = engine.suggestions(&rules).unwrap();
    assert!(suggestions.iter().all(|s| !s.name.contains("张建国")
        && !s.name.contains("王立群")
        && !s.name.ends_with(':')
        && !s.name.ends_with('：')));
}

