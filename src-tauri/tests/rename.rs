use excel_workbench::{model::SheetRange, rename::*};
use rust_xlsxwriter::Workbook;
use std::{fs, path::Path};
use tempfile::TempDir;

fn cell(sheet: &str, row: usize, column: usize) -> RenameCell { RenameCell { sheet:sheet.into(),row,column } }
fn isolated() -> TempDir { TempDir::new_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap() }
fn fixture(path: &Path, code: &str) {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet(); sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,code).unwrap(); sheet.write_string(1,0,"填报人").unwrap();
    sheet.write_number(2,0,120.).unwrap(); sheet.write_number(3,0,3.).unwrap(); sheet.write_number(4,0,1.).unwrap();
    let sheet = book.add_worksheet(); sheet.set_name("明细").unwrap();
    sheet.write_string(0,0,"数量").unwrap(); sheet.write_string(0,1,"金额").unwrap();
    sheet.write_number(1,0,1000.).unwrap(); sheet.write_number(1,1,40.).unwrap();
    sheet.write_number(2,0,2000.).unwrap(); sheet.write_number(2,1,60.).unwrap();
    // An incomplete row must be excluded by the configurable filled-cell threshold.
    sheet.write_number(3,1,999.).unwrap();
    let sheet = book.add_worksheet(); sheet.set_name("附加").unwrap(); sheet.write_number(0,0,20.).unwrap();
    book.save(path).unwrap();
}
fn rules() -> RenameRules {
    RenameRules { parts:vec![RenamePart::Cell { source:cell("说明",1,1),decimals:None }],separator:"_".into(),prefix:String::new(),suffix:String::new(),checks:vec![] }
}

#[test]
fn rename_reads_only_configured_cells_and_check_areas() {
    let temp=isolated();let path=temp.path().join("无关超宽列.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,"001").unwrap();sheet.write_number(1,0,30.).unwrap();sheet.write_number(2,0,10.).unwrap();sheet.write_number(3,0,20.).unwrap();
    sheet.write_string(3,4095,"新增列头").unwrap();sheet.write_number(99,4095,999.).unwrap();sheet.write_formula(0,16383,rust_xlsxwriter::Formula::new("=1+1").set_result("2")).unwrap();book.save(&path).unwrap();let original=fs::read(&path).unwrap();
    let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);let mut r=rules();r.checks.push(RenameCheck {label:"金额异常".into(),expected:cell("说明",2,1),sums:vec![RenameSum {area:RenameArea {sheet:"说明".into(),range:SheetRange {start_row:3,end_row:4,start_column:1,end_column:1}},divisor:1.,min_filled:0,condition:None}],tolerance_percent:0.});
    let live=service.live(&file.id,&r).unwrap();assert_eq!(live.state,"ready","unrelated cells must not block renaming: {}",live.message);assert_eq!(live.new_name,"001.xlsx");assert!(live.checks[0].contains("通过"));
    let mut far=r.clone();far.parts.push(RenamePart::Cell {source:cell("说明",1,16384),decimals:None});assert_eq!(service.live(&file.id,&far).unwrap().new_name,"001_2.xlsx");
    let plan=service.prepare(vec![file.id.clone()],r).unwrap();assert_eq!(plan.rows[0].state,"ready");
    let page=service.page_window(&file.id,"说明",0,100,16320).unwrap();assert_eq!(page.column_offset,16320);assert_eq!(page.sheet.width,64);assert_eq!(page.page.rows[0][63],excel_workbench::model::Cell::Number(2.));assert_eq!(fs::read(&path).unwrap(),original);
}

#[test]
fn trailing_blank_columns_do_not_skip_rename_or_summary() {
    let temp=isolated();let path=temp.path().join("误扩空列.xlsx");
    let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,"001").unwrap();sheet.write_number(4,4,123.45).unwrap();
    sheet.write_blank(8,16383,&rust_xlsxwriter::Format::new().set_background_color("#EEEEEE")).unwrap();
    sheet.write_string(1_048_575,16383,"   \t").unwrap();book.save(&path).unwrap();
    let original=fs::read(&path).unwrap();
    let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);
    let row=service.live(&file.id,&rules()).unwrap();assert_eq!(row.state,"ready","blank tail must not skip the file: {}",row.message);assert_eq!(row.new_name,"001.xlsx");
    let page=service.page(&file.id,"说明",0,100).unwrap();assert_eq!((page.sheet.rows,page.sheet.width),(1_048_576,80));assert_eq!(page.page.rows.len(),100);assert_eq!(page.page.rows[4][4],excel_workbench::model::Cell::Number(123.45));
    let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!((imported.sheets[0].rows.len(),imported.sheets[0].width),(5,5));
    assert_eq!(fs::read(&path).unwrap(),original,"reading must not modify the original workbook");
}

#[test]
fn expanded_excel_table_headers_without_column_data_do_not_block_reading() {
    let temp=isolated();let path=temp.path().join("超级表误扩列.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();
    sheet.write_string(1,0,"001").unwrap();sheet.write_string(1,1,"填报人").unwrap();sheet.write_number(1,2,10.).unwrap();sheet.write_number(4,2,20.).unwrap();
    sheet.add_table(0,0,5,4095,&rust_xlsxwriter::Table::new()).unwrap();book.save(&path).unwrap();let original=fs::read(&path).unwrap();
    let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);let mut r=rules();r.parts[0]=RenamePart::Cell {source:cell("说明",2,1),decimals:None};
    let live=service.live(&file.id,&r).unwrap();assert_eq!(live.state,"ready","table-generated headers without body data should not block reading: {}",live.message);assert_eq!(live.new_name,"001.xlsx");
    let page=service.page(&file.id,"说明",0,100).unwrap();assert_eq!(page.sheet.width,80);assert!(page.appearance.note.as_ref().is_some_and(|n|n.contains("按行加载")),"preview should disclose its bounded window");
    let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!(imported.sheets[0].width,3);assert_eq!(fs::read(&path).unwrap(),original);
    let core=excel_workbench::engine::Engine::open(&temp.path().join("data")).unwrap();let imported=core.import_files(vec![path.to_string_lossy().into()]).unwrap();assert!(imported[0].error.is_none());let source=&imported[0].file.as_ref().unwrap().sheets[0];
    assert_eq!(source.width,3);assert!(core.preview_appearance(&source.id,0,100).unwrap().note.as_ref().is_some_and(|n|n.contains("4093")));
}

#[test]
fn table_cropping_preserves_real_tail_values_and_regular_headers() {
    let temp=isolated();
    for kind in ["number","false","formula","text"] {
        let path=temp.path().join(format!("真实尾列-{kind}.xlsx"));let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();sheet.write_string(1,0,"001").unwrap();sheet.add_table(0,0,5,4095,&rust_xlsxwriter::Table::new()).unwrap();
        match kind {"number"=>{sheet.write_number(2,4095,0.).unwrap();},"false"=>{sheet.write_boolean(2,4095,false).unwrap();},"formula"=>{sheet.write_formula(2,4095,rust_xlsxwriter::Formula::new("IF(1=1,\"\",\"\")").set_result("")).unwrap();},_=>{sheet.write_string(2,4095,"有内容").unwrap();}}
        book.save(&path).unwrap();assert_eq!(excel_workbench::reader::read(&path).err().unwrap().code,"LIMIT","{kind} must not be cropped");
    }
    let path=temp.path().join("普通表头.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.write_string(0,4095,"普通文本").unwrap();sheet.write_string(1,0,"001").unwrap();book.save(&path).unwrap();assert_eq!(excel_workbench::reader::read(&path).err().unwrap().code,"LIMIT","ordinary text is not presumed to be a generated table header");
    let path=temp.path().join("正常小表.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.write_string(1,0,"001").unwrap();sheet.add_table(0,0,5,5,&rust_xlsxwriter::Table::new()).unwrap();book.save(&path).unwrap();let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!(imported.sheets[0].width,6);assert!(imported.sheets[0].note.is_none());
}

#[test]
fn offset_and_empty_oversized_tables_keep_original_coordinates() {
    let temp=isolated();let path=temp.path().join("偏移超级表.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,"表外说明").unwrap();sheet.write_string(5,2,"001").unwrap();sheet.write_number(6,4,1.).unwrap();sheet.add_table(4,2,10,4095,&rust_xlsxwriter::Table::new()).unwrap();book.save(&path).unwrap();
    let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!((imported.sheets[0].rows.len(),imported.sheets[0].width),(7,5));assert_eq!(imported.sheets[0].rows[5][2],excel_workbench::model::Cell::Text("001".into()));assert!(matches!(imported.sheets[0].rows[0][2],excel_workbench::model::Cell::Empty));
    let path=temp.path().join("只有表头.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.add_table(0,0,1,4095,&rust_xlsxwriter::Table::new()).unwrap();book.save(&path).unwrap();let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!((imported.sheets[0].rows.len(),imported.sheets[0].width),(1,1));assert!(imported.sheets[0].note.is_some());
}

#[test]
fn absolute_and_parent_relative_table_links_read_the_same_body() {
    use std::io::{Cursor,Read,Write};
    let temp=isolated();let original=temp.path().join("原始路径.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.write_string(1,0,"001").unwrap();sheet.write_number(2,2,20.).unwrap();sheet.add_table(0,0,5,4095,&rust_xlsxwriter::Table::new()).unwrap();book.save(&original).unwrap();let original=fs::read(original).unwrap();
    for (index,target) in ["/xl/tables/table1.xml","../../xl/tables/table1.xml"].into_iter().enumerate() {
        let mut source=zip::ZipArchive::new(Cursor::new(original.clone())).unwrap();let mut output=zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..source.len() {
            let mut file=source.by_index(i).unwrap();let name=file.name().to_owned();let mut bytes=Vec::new();file.read_to_end(&mut bytes).unwrap();
            if name=="xl/worksheets/_rels/sheet1.xml.rels" {let xml=String::from_utf8(bytes).unwrap();assert!(xml.contains("../tables/table1.xml"));bytes=xml.replace("../tables/table1.xml",target).into_bytes();}
            output.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();output.write_all(&bytes).unwrap();
        }
        let path=temp.path().join(format!("路径{index}.xlsx"));fs::write(&path,output.finish().unwrap().into_inner()).unwrap();let imported=excel_workbench::reader::read(&path).unwrap();assert_eq!(imported.sheets[0].width,3,"{target}");assert!(imported.sheets[0].note.as_ref().is_some_and(|n|n.contains("4093")));
    }
}

#[test]
fn distant_real_values_and_empty_result_formulas_are_never_silently_trimmed() {
    let temp=isolated();
    for kind in ["zero","false","formula"] {
        let path=temp.path().join(format!("{kind}.xlsx"));let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();sheet.write_string(0,0,"001").unwrap();
        match kind {
            "zero"=>{sheet.write_number(0,2048,0.).unwrap();},
            "false"=>{sheet.write_boolean(0,2048,false).unwrap();},
            _=>{sheet.write_formula(0,2048,rust_xlsxwriter::Formula::new("IF(1=1,\"\",\"\")").set_result("")).unwrap();},
        }
        book.save(&path).unwrap();let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);
        let row=service.live(&file.id,&rules()).unwrap();assert_eq!(row.state,"ready","unreferenced {kind} must not block renaming: {}",row.message);
        assert_eq!(excel_workbench::reader::read(&path).err().unwrap().code,"LIMIT");
    }
}

#[test]
fn cached_empty_formula_results_keep_their_original_meaning() {
    let temp=isolated();let path=temp.path().join("空结果公式.xlsx");let mut book=Workbook::new();let sheet=book.add_worksheet();sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,"001").unwrap();sheet.write_formula(0,1,rust_xlsxwriter::Formula::new("IF(1=1,\"\",\"\")").set_result("")).unwrap();book.save(&path).unwrap();
    // The writer substitutes numeric zero for an empty supplied result. Use Excel's actual cached-string representation.
    {
        use std::io::{Cursor,Read,Write};
        let mut source=zip::ZipArchive::new(Cursor::new(fs::read(&path).unwrap())).unwrap();let mut output=zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..source.len() {
            let mut file=source.by_index(i).unwrap();let name=file.name().to_owned();let mut bytes=Vec::new();file.read_to_end(&mut bytes).unwrap();
            if name=="xl/worksheets/sheet1.xml" {
                let xml=String::from_utf8(bytes).unwrap();assert!(xml.contains("<c r=\"B1\">"));assert!(xml.contains("</f><v>0</v>"));
                bytes=xml.replace("<c r=\"B1\">","<c r=\"B1\" t=\"str\">").replace("</f><v>0</v>","</f><v></v>").into_bytes();
            }
            output.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();output.write_all(&bytes).unwrap();
        }
        fs::write(&path,output.finish().unwrap().into_inner()).unwrap();
    }
    let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);
    let page=service.page(&file.id,"说明",0,100).unwrap();assert_eq!(page.sheet.width,2);assert_eq!(page.page.rows[0][1],excel_workbench::model::Cell::Text(String::new()));
    let mut r=rules();r.parts[0]=RenamePart::Condition {condition:RenameCondition {source:cell("说明",1,2),operator:"nonempty".into(),value:String::new()},text:"标签".into()};r.prefix="文件".into();
    let row=service.live(&file.id,&r).unwrap();assert_eq!(row.state,"ready","cached empty results must not become missing-cache errors: {}",row.message);assert_eq!(row.new_name,"文件.xlsx");
    assert_eq!(excel_workbench::reader::read(&path).unwrap().sheets[0].rows[0][1],excel_workbench::model::Cell::Text(String::new()));
}
fn output(mode:RenameOutputMode,directory:&Path)->RenameOutput {RenameOutput {mode,directory:Some(directory.to_string_lossy().into()),number_conflicts:true}}

#[test]
fn collect_copies_numbers_conflicts_and_undo_preserves_originals() {
    let temp=isolated();let inputs=temp.path().join("多层来源");let destination=temp.path().join("集中结果");fs::create_dir_all(inputs.join("甲")).unwrap();fs::create_dir_all(inputs.join("乙")).unwrap();fs::create_dir(&destination).unwrap();
    let a=inputs.join("甲/原名.xlsx");let b=inputs.join("乙/原名.xlsx");fixture(&a,"统一名称");fixture(&b,"统一名称");
    let bytes=fs::read(&a).unwrap();fs::write(destination.join("统一名称.xlsx"),b"existing-output").unwrap();
    let service=RenameService::default();let files=service.scan(vec![inputs.to_string_lossy().into()],true).unwrap().files;
    let plan=service.prepare_to(files.iter().map(|f|f.id.clone()).collect(),rules(),output(RenameOutputMode::Copy,&destination)).unwrap();
    assert_eq!(plan.rows.iter().map(|r|r.new_name.as_str()).collect::<Vec<_>>(),["统一名称 (2).xlsx","统一名称 (3).xlsx"]);
    assert!(plan.rows.iter().all(|r|r.state=="ready" && r.target_path.as_ref().unwrap().contains("集中结果")));
    let done=service.execute(&plan.id,temp.path()).unwrap();assert!(done.rows.iter().all(|r|r.state=="copied"));
    assert_eq!(fs::read(&a).unwrap(),bytes);assert_eq!(fs::read(&b).unwrap(),bytes);
    for row in &done.rows {assert_eq!(fs::read(row.target_path.as_ref().unwrap()).unwrap(),bytes);}
    let restarted=RenameService::default();assert_eq!(restarted.latest(temp.path()).unwrap().unwrap().rows[0].state,"copied");
    let undone=restarted.undo(&done.id,temp.path()).unwrap();assert!(undone.rows.iter().all(|r|r.state=="restored"));
    assert_eq!(fs::read(destination.join("统一名称.xlsx")).unwrap(),b"existing-output");assert!(a.exists() && b.exists());
}

#[test]
fn collected_move_restores_original_folders_and_copy_undo_guards_changed_files() {
    let temp=isolated();let inputs=temp.path().join("来源");let destination=temp.path().join("结果");fs::create_dir_all(inputs.join("层级")).unwrap();fs::create_dir(&destination).unwrap();
    let path=inputs.join("层级/相同.xlsx");fixture(&path,"相同");let bytes=fs::read(&path).unwrap();
    let service=RenameService::default();let file=service.scan(vec![path.to_string_lossy().into()],true).unwrap().files.remove(0);
    let plan=service.prepare_to(vec![file.id.clone()],rules(),output(RenameOutputMode::Move,&destination)).unwrap();assert_eq!(plan.rows[0].state,"ready","same filename in a different folder must still move");
    let done=service.execute(&plan.id,temp.path()).unwrap();assert_eq!(done.rows[0].state,"moved");assert!(!path.exists());
    fs::remove_dir_all(&inputs).unwrap();let undone=service.undo(&done.id,temp.path()).unwrap();assert_eq!(undone.rows[0].state,"restored");assert_eq!(fs::read(&path).unwrap(),bytes);
    let plan=service.prepare_to(vec![file.id.clone()],rules(),output(RenameOutputMode::Copy,&destination)).unwrap();
    fs::write(destination.join("相同.xlsx"),b"race-after-preview").unwrap();let failed=service.execute(&plan.id,temp.path()).unwrap();assert_eq!(failed.rows[0].state,"failed");assert_eq!(fs::read(destination.join("相同.xlsx")).unwrap(),b"race-after-preview");assert!(path.exists());
    let plan=service.prepare_to(vec![file.id],rules(),output(RenameOutputMode::Copy,&destination)).unwrap();let done=service.execute(&plan.id,temp.path()).unwrap();assert_eq!(done.rows[0].state,"copied");
    fs::remove_file(&path).unwrap();let undone=service.undo(&done.id,temp.path()).unwrap();assert_eq!(undone.rows[0].state,"undoFailed");assert_eq!(fs::read(done.rows[0].target_path.as_ref().unwrap()).unwrap(),bytes,"the surviving copy must never be deleted");
}

#[cfg(windows)]
#[test]
#[ignore = "requires an explicitly supplied, disposable directory on a second drive"]
fn cross_volume_move_and_readonly_source_are_recoverable() {
    let target_root=std::env::var("GRIDFLOW_RENAME_TEST_DESTINATION").expect("set a disposable directory on a second drive");
    let temp=isolated();let destination=TempDir::new_in(target_root).unwrap();
    assert_ne!(temp.path().to_string_lossy().chars().next(),destination.path().to_string_lossy().chars().next(),"this check requires two actual drives");
    let source=temp.path().join("跨盘来源.xlsx");fixture(&source,"跨盘结果");let bytes=fs::read(&source).unwrap();
    let service=RenameService::default();let file=service.scan(vec![source.to_string_lossy().into()],false).unwrap().files.remove(0);
    let plan=service.prepare_to(vec![file.id.clone()],rules(),output(RenameOutputMode::Move,destination.path())).unwrap();
    let done=service.execute(&plan.id,temp.path()).unwrap();assert_eq!(done.rows[0].state,"moved");assert!(!source.exists());assert_eq!(fs::read(done.rows[0].target_path.as_ref().unwrap()).unwrap(),bytes);
    let undone=service.undo(&done.id,temp.path()).unwrap();assert_eq!(undone.rows[0].state,"restored");assert_eq!(fs::read(&source).unwrap(),bytes);
    let mut permissions=fs::metadata(&source).unwrap().permissions();permissions.set_readonly(true);fs::set_permissions(&source,permissions).unwrap();
    let plan=service.prepare_to(vec![file.id],rules(),output(RenameOutputMode::Move,destination.path())).unwrap();
    let done=service.execute(&plan.id,temp.path()).unwrap();assert_eq!(done.rows[0].state,"copyOnly","a source removal failure must preserve and report both files");assert!(source.exists());
    let mut permissions=fs::metadata(&source).unwrap().permissions();permissions.set_readonly(false);fs::set_permissions(&source,permissions).unwrap();
    let undone=service.undo(&done.id,temp.path()).unwrap();assert_eq!(undone.rows[0].state,"restored");assert_eq!(fs::read(&source).unwrap(),bytes);assert!(!Path::new(done.rows[0].target_path.as_ref().unwrap()).exists());
    // A blocked restore must be retryable after the user resolves the conflict.
    let file=service.files().remove(0);let plan=service.prepare_to(vec![file.id],rules(),output(RenameOutputMode::Move,destination.path())).unwrap();let done=service.execute(&plan.id,temp.path()).unwrap();
    let target=Path::new(done.rows[0].target_path.as_ref().unwrap());let mut permissions=fs::metadata(target).unwrap().permissions();permissions.set_readonly(true);fs::set_permissions(target,permissions).unwrap();
    let interrupted=service.undo(&done.id,temp.path()).unwrap();assert_eq!(interrupted.rows[0].state,"undoFailed");assert!(source.exists() && target.exists());
    let mut permissions=fs::metadata(target).unwrap().permissions();permissions.set_readonly(false);fs::set_permissions(target,permissions).unwrap();
    let retried=service.undo(&done.id,temp.path()).unwrap();assert_eq!(retried.rows[0].state,"restored");assert!(!target.exists());assert_eq!(fs::read(&source).unwrap(),bytes);
}
fn sum(sheet: &str, column: usize, divisor: f64, min_filled: usize) -> RenameSum {
    RenameSum { area:RenameArea { sheet:sheet.into(),range:SheetRange { start_row:2,end_row:1048576,start_column:column,end_column:column } },divisor,min_filled,condition:None }
}

#[test]
fn optional_lookup_checks_and_conditions_use_actual_workbooks() {
    let temp = isolated(); let path = temp.path().join("原名.xlsx"); fixture(&path,"001");
    let original = fs::read(&path).unwrap();
    let service = RenameService::default(); let files = service.scan(vec![path.to_string_lossy().into()],true).unwrap().files;
    let mut r = rules();
    r.parts.push(RenamePart::Lookup { source:cell("说明",1,1),name:"客户简称".into(),entries:vec![RenameLookupEntry { key:"001".into(),value:"客户甲".into() }],fallback:None });
    r.parts.push(RenamePart::Cell { source:cell("说明",3,1),decimals:Some(2) });
    r.parts.push(RenamePart::Condition { condition:RenameCondition { source:cell("说明",5,1),operator:"positive".into(),value:String::new() },text:"【CPU】".into() }); r.suffix = "万元".into();
    let extra = RenameSum { area:RenameArea { sheet:"附加".into(),range:SheetRange { start_row:1,end_row:1,start_column:1,end_column:1 } },divisor:1.,min_filled:0,condition:Some(RenameCondition { source:cell("说明",5,1),operator:"positive".into(),value:String::new() }) };
    r.checks = vec![RenameCheck { label:"金额异常".into(),expected:cell("说明",3,1),sums:vec![sum("明细",2,1.,2),extra],tolerance_percent:1. }, RenameCheck { label:"数量异常".into(),expected:cell("说明",4,1),sums:vec![sum("明细",1,1000.,2)],tolerance_percent:1. }];
    let live = service.live(&files[0].id,&r).unwrap();
    assert_eq!(live.new_name,"001_客户甲_120.00_【CPU】万元.xlsx"); assert!(live.checks.iter().all(|c| c.contains("通过")));
    assert!(live.checks[0].contains("合计 120.00 / 填写 120.00，容差 1.00%"));
    r.checks[0].sums.remove(1);
    assert!(service.live(&files[0].id,&r).unwrap().new_name.starts_with("金额异常_"));
    assert!(service.live(&files[0].id,&r).unwrap().has_mismatch);
    assert_eq!(fs::read(&path).unwrap(),original,"previews must never rewrite Excel content");
    r.parts.push(RenamePart::Cell { source:cell("缺少工作表",1,1),decimals:None });
    assert_eq!(service.live(&files[0].id,&r).unwrap().state,"skipped");
}

#[test]
fn check_display_rounds_to_two_decimals_without_rounding_the_comparison() {
    let temp=isolated(); let path=temp.path().join("显示精度.xlsx");
    let mut book=Workbook::new(); let sheet=book.add_worksheet(); sheet.set_name("说明").unwrap();
    sheet.write_string(0,0,"示例").unwrap(); sheet.write_number(1,0,1.0001).unwrap(); sheet.write_number(2,0,1.0002).unwrap(); book.save(&path).unwrap();
    let service=RenameService::default(); let file=service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);
    let mut r=rules(); r.checks.push(RenameCheck {label:"差异".into(),expected:cell("说明",2,1),sums:vec![RenameSum {area:RenameArea {sheet:"说明".into(),range:SheetRange {start_row:3,end_row:3,start_column:1,end_column:1}},divisor:1.,min_filled:0,condition:None}],tolerance_percent:0.});
    let row=service.live(&file.id,&r).unwrap();
    assert_eq!(row.checks[0],"差异：不一致（合计 1.00 / 填写 1.00，容差 0.00%）");
    assert!(row.has_mismatch,"comparison must retain precision even when displayed numbers round to the same value");
}

#[test]
fn rename_never_overwrites_and_can_recover_after_restart() {
    let temp = isolated(); let path = temp.path().join("原名.xlsx"); fixture(&path,"001");
    let original = fs::read(&path).unwrap(); let service = RenameService::default();
    let file = service.scan(vec![path.to_string_lossy().into()],false).unwrap().files.remove(0);
    let plan = service.prepare(vec![file.id.clone()],rules()).unwrap();
    let target = path.with_file_name("001.xlsx");
    fs::write(&target,b"another file created AFTER preview").unwrap();
    let run = service.execute(&plan.id,temp.path()).unwrap(); assert_eq!(run.rows[0].state,"failed");
    assert_eq!(fs::read(&target).unwrap(),b"another file created AFTER preview"); assert_eq!(fs::read(&path).unwrap(),original);
    fs::remove_file(&target).unwrap();
    let plan = service.prepare(vec![file.id],rules()).unwrap();
    let run = service.execute(&plan.id,temp.path()).unwrap(); assert_eq!(run.rows[0].state,"renamed", "{:?}", run.rows);
    assert!(!path.exists()); assert_eq!(fs::read(&target).unwrap(),original);
    fs::write(&path,b"first original-name conflict").unwrap();
    assert_eq!(service.undo(&run.id,temp.path()).unwrap().rows[0].state,"undoFailed");
    assert_eq!(fs::read(&path).unwrap(),b"first original-name conflict"); fs::remove_file(&path).unwrap();
    // Model termination after the move: durable intent exists, final UTF-8 record is torn.
    let journal = run.journal_path.as_ref().unwrap(); let bytes = fs::read(journal).unwrap();
    let mut interrupted = bytes.split(|b| *b == b'\n').take(2).flat_map(|line| line.iter().copied().chain([b'\n'])).collect::<Vec<_>>();
    interrupted.extend_from_slice(b"{\"unfinished\":\""); interrupted.push(0xe4); fs::write(journal, interrupted).unwrap();
    let restarted = RenameService::default(); let recovered = restarted.latest(temp.path()).unwrap().unwrap(); assert_eq!(recovered.id,run.id);
    assert_eq!(recovered.rows[0].state,"renamed");
    fs::write(&path,b"new file occupying original name").unwrap();
    let blocked = restarted.undo(&run.id,temp.path()).unwrap(); assert_eq!(blocked.rows[0].state,"uncertain");
    assert_eq!(fs::read(&path).unwrap(),b"new file occupying original name"); assert_eq!(fs::read(&target).unwrap(),original);
    fs::remove_file(&path).unwrap(); let restored = restarted.undo(&run.id,temp.path()).unwrap(); assert_eq!(restored.rows[0].state,"restored");
    assert_eq!(fs::read(&path).unwrap(),original); assert!(!target.exists());
    assert_eq!(restarted.latest(temp.path()).unwrap().unwrap().rows[0].state,"restored");
    let export = temp.path().join("日志.xlsx"); restarted.export(&restored,&export).unwrap(); assert!(export.metadata().unwrap().len()>0);
    assert_eq!(excel_workbench::reader::read(&export).unwrap().sheets.len(), 2);
    assert!(restarted.export(&restored,&export).is_err(),"existing exported files must not be overwritten");
}

#[test]
fn duplicates_and_changed_sources_are_skipped_without_stopping_other_files() {
    let temp = isolated();
    let a=temp.path().join("甲.xlsx"); let b=temp.path().join("乙.xlsx"); let c=temp.path().join("丙.xlsx");
    fixture(&a,"相同"); fixture(&b,"相同"); fixture(&c,"独立");
    let service=RenameService::default(); let files=service.scan(vec![temp.path().to_string_lossy().into()],true).unwrap().files;
    assert_eq!(service.scan(vec![temp.path().to_string_lossy().into()],true).unwrap().files.iter().map(|f| &f.id).collect::<Vec<_>>(), files.iter().map(|f| &f.id).collect::<Vec<_>>(), "rescanning must preserve IDs used by the selection");
    let plan=service.prepare(files.iter().map(|f|f.id.clone()).collect(),rules()).unwrap();
    assert_eq!(plan.rows.iter().filter(|r|r.state=="skipped").count(),2);
    assert_eq!(plan.rows.iter().filter(|r|r.state=="ready").count(),1);
    fs::write(&c,b"source changed after preview").unwrap();
    let run=service.execute(&plan.id,temp.path()).unwrap();
    assert_eq!(run.rows.iter().filter(|r|r.state=="failed").count(),1);
    assert!(a.exists()&&b.exists()&&c.exists()); assert!(!temp.path().join("独立.xlsx").exists());
}
