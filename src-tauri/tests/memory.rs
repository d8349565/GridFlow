use excel_workbench::{engine::Engine, model::*};
use rust_xlsxwriter::Workbook;
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

#[test]
fn three_hundred_workbooks_can_be_imported_and_merged_without_losing_rows() {
    let temp = TempDir::new().unwrap();
    let mut paths = Vec::new();
    for file in 0..300 {
        let mut book = Workbook::new();
        let sheet = book.add_worksheet();
        sheet.set_name("明细").unwrap();
        sheet.write_string(0, 0, "编号").unwrap();
        for column in 1..24u16 {
            sheet.write_string(0, column, format!("数值{column}")).unwrap();
        }
        for row in 1..=8u32 {
            sheet.write_string(row, 0, format!("{file:03}-{row:03}")).unwrap();
            for column in 1..24u16 {
                sheet.write_number(row, column, row as f64).unwrap();
            }
        }
        let path = temp.path().join(format!("表格-{file:03}.xlsx"));
        book.save(&path).unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let original = std::fs::read(&paths[299]).unwrap();
    let engine = Engine::open(&temp.path().join("isolated-data")).unwrap();
    let imported = engine.import_files(paths.clone()).unwrap();
    let errors = imported.iter().filter_map(|item| item.error.as_ref()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{} imports failed; first: {:?}", errors.len(), errors.first());
    let mut rules = engine.snapshot().unwrap().rules;
    rules.merge_mode = Some("single".into());
    rules.include_source = false;
    let result = engine.process(&rules, false, &AtomicBool::new(false), |_, _| {}).unwrap();
    assert_eq!(result.summary.output_rows, 2400);
    assert_eq!(result.columns.len(), 24);
    assert_eq!(std::fs::read(&paths[299]).unwrap(), original);
}

#[test]
fn supported_wide_sheet_keeps_real_tail_data_without_running_out_of_memory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("宽表.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet_with_constant_memory();
    for column in 0..1024u16 {
        sheet.write_string(0, column, format!("字段{column}")).unwrap();
    }
    for row in 1..=1024u32 {
        for column in 0..1024u16 {
            if column == 0 { sheet.write_string(row, column, "001").unwrap(); }
            else if column == 1023 { sheet.write_number(row, column, 42.).unwrap(); }
            else { sheet.write_string(row, column, format!("记录-{row:04}-{column:04}-用于核对完整内容不丢失的宽表文本字段")).unwrap(); }
        }
    }
    book.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let engine = Engine::open(&temp.path().join("isolated-data")).unwrap();
    let imported = engine.import_files(vec![path.to_string_lossy().into_owned()]).unwrap();
    assert!(imported[0].error.is_none(), "{:?}", imported[0].error);
    let source = &imported[0].file.as_ref().unwrap().sheets[0];
    let page = engine.preview_sheet(&source.id, 1, 1, None).unwrap();
    assert_eq!(page.rows[0][0], Cell::Text("001".into()));
    assert_eq!(page.rows[0][1023], Cell::Number(42.));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn cancelling_a_batch_keeps_completed_files_and_allows_the_next_import() {
    let temp = TempDir::new().unwrap();
    let mut paths = Vec::new();
    for file in 0..300 {
        let path = temp.path().join(format!("可取消-{file}.csv"));
        std::fs::write(&path, "编号,金额\n001,12.5\n002,0\n").unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let engine = Engine::open(&temp.path().join("isolated-data")).unwrap();
    let worker = engine.clone();
    let inputs = paths.clone();
    let job = std::thread::spawn(move || worker.import_files(inputs));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let progress = engine.import_progress().unwrap();
        if progress.busy && progress.done > 0 {
            engine.cancel_import().unwrap();
            break;
        }
        assert!(!job.is_finished(), "the import must expose progress before finishing");
        assert!(std::time::Instant::now() < deadline, "import progress did not arrive");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let report = job.join().unwrap().unwrap();
    assert!(!report.is_empty() && report.len() < 300);
    assert!(report.iter().all(|item| item.error.is_none()));
    assert_eq!(engine.snapshot().unwrap().files.len(), report.len());
    let progress = engine.import_progress().unwrap();
    assert!(!progress.busy && progress.cancelled);
    let retry = engine.import_files(vec![paths[299].clone()]).unwrap();
    assert!(retry[0].error.is_none());
    let sheet = &retry[0].file.as_ref().unwrap().sheets[0];
    assert_eq!(engine.preview_sheet(&sheet.id, 1, 1, None).unwrap().rows[0][0], Cell::Text("001".into()));
}

#[test]
fn cancelling_inside_a_workbook_does_not_publish_half_a_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("多表工作簿.xlsx");
    let mut book = Workbook::new();
    for _ in 0..12 {
        let sheet = book.add_worksheet_with_constant_memory();
        for column in 0..20u16 { sheet.write_string(0, column, format!("字段{column}")).unwrap(); }
        for row in 1..=2000u32 {
            for column in 0..20u16 { sheet.write_number(row, column, row as f64).unwrap(); }
        }
    }
    book.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let engine = Engine::open(&temp.path().join("isolated-data")).unwrap();
    let worker = engine.clone();
    let input = path.to_string_lossy().into_owned();
    let job = std::thread::spawn(move || worker.import_files(vec![input]));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !engine.import_progress().unwrap().busy {
        assert!(!job.is_finished() && std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    engine.cancel_import().unwrap();
    let report = job.join().unwrap().unwrap();
    assert!(report.is_empty(), "a cancelled workbook must not become a completed import");
    assert!(engine.snapshot().unwrap().files.is_empty());
    assert!(engine.import_progress().unwrap().cancelled);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn expanded_table_with_large_blank_formatting_keeps_data_and_preview_format() {
    use std::io::{Read, Write};
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("误扩超级表.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.write_number_with_format(1, 0, 23., &rust_xlsxwriter::Format::new().set_num_format("00000")).unwrap();
    sheet.write_string(1, 1, "有效数据").unwrap();
    sheet.write_number(1, 2, 1.5).unwrap();
    sheet.add_table(0, 0, 2, 4095, &rust_xlsxwriter::Table::new()).unwrap();
    let mut source = zip::ZipArchive::new(std::io::Cursor::new(book.save_to_buffer().unwrap())).unwrap();
    let mut output = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().to_owned();
        output.start_file(&name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            let mut xml = String::new(); entry.read_to_string(&mut xml).unwrap();
            let (head, tail) = xml.split_once("</sheetData>").unwrap();
            output.write_all(head.as_bytes()).unwrap();
            // A real mis-expansion pattern: many styled, empty cells, no business values.
            let mut bytes = 0;
            for row in 4..=150_000 {
                let mut line = format!("<row r=\"{row}\">");
                for column in 1..=32 { line.push_str(&format!("<c r=\"{}\" s=\"1\"/>", excel_workbench::reader::cell_address(row, column))); }
                line.push_str("</row>");
                bytes += line.len();
                output.write_all(line.as_bytes()).unwrap();
            }
            assert!(bytes > 80_000_000);
            output.write_all(b"</sheetData>").unwrap();
            output.write_all(tail.as_bytes()).unwrap();
        } else if name == "xl/tables/table1.xml" {
            let mut xml = String::new(); entry.read_to_string(&mut xml).unwrap();
            let old = format!("A1:{}", excel_workbench::reader::cell_address(3, 4096));
            let expanded = format!("A1:{}", excel_workbench::reader::cell_address(150_000, 4096));
            output.write_all(xml.replace(&old, &expanded).as_bytes()).unwrap();
        } else { std::io::copy(&mut entry, &mut output).unwrap(); }
    }
    output.finish().unwrap();
    let original = std::fs::read(&path).unwrap();
    let engine = Engine::open(&temp.path().join("isolated-data")).unwrap();
    let imported = engine.import_files(vec![path.to_string_lossy().into_owned()]).unwrap();
    assert!(imported[0].error.is_none(), "{:?}", imported[0].error);
    let sheet = &imported[0].file.as_ref().unwrap().sheets[0];
    assert_eq!((sheet.rows, sheet.width), (2, 3));
    let preview = engine.preview_appearance(&sheet.id, 0, 100).unwrap();
    assert!(preview.styles.iter().any(|style| style.number_format == "00000"), "valid formatting must survive the blank expansion: {:?}", preview.note);
    let values = engine.preview_sheet(&sheet.id, 1, 1, None).unwrap();
    assert_eq!(values.rows[0], vec![Cell::Number(23.), Cell::Text("有效数据".into()), Cell::Number(1.5)]);
    assert_eq!(std::fs::read(path).unwrap(), original);
}
