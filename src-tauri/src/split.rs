//! Independent table splitting. A prepared plan owns a private row-order snapshot;
//! filenames, membership and output columns used by export are the reviewed plan.
use crate::{
    analysis::{index, order},
    engine::{append_cells, cell_from_row, create_table, padded_identifier, read_rows, Engine},
    export::{export_conditions, export_rows, temporary, write_rows},
    model::*, reader,
};
use duckdb::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Mutex}};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SplitNamePart {
    Value, Text { text: String }, Source, Sheet, Date, Sequence,
    Column { key: String },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitRequest {
    pub query: DatasetQuery,
    pub field: String,
    pub columns: Vec<String>,
    pub naming: Vec<SplitNamePart>,
    pub separator: String,
    pub format: String,
    pub source_name: String,
    pub sheet_name: String,
    pub date: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitOutput {
    pub directory: String,
    pub new_folder: bool,
    pub folder_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitEdit {
    pub id: String,
    pub selected: bool,
    pub name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitGroup {
    pub id: String,
    pub value: String,
    pub kind: u8,
    pub rows: usize,
    pub selected: bool,
    pub filename: String,
    pub note: String,
    pub error: Option<String>,
    pub state: String,
    pub path: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitPlan {
    pub id: String,
    pub groups: Vec<SplitGroup>,
    pub total_rows: usize,
    pub selected_rows: usize,
    pub directory: Option<String>,
    pub note: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitProgress {
    pub busy: bool,
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub current: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SplitSourcePage {
    pub page: DataPage,
    pub appearance: SheetAppearance,
}

struct Imported { file: SourceFile, dataset: Option<Dataset> }
struct Prepared {
    plan: SplitPlan,
    conn: Connection,
    dataset: Dataset,
    indices: Vec<usize>,
    grouping_key: String,
    members: Vec<(String, u8, usize)>,
    create_directory: bool,
    started: bool,
}
#[derive(Default)]
pub struct SplitService {
    imported: Mutex<Option<Imported>>,
    prepared: Mutex<Option<Prepared>>,
    progress: Mutex<SplitProgress>,
    cancelled: AtomicBool,
    busy: AtomicBool,
}
struct Job<'a>(&'a SplitService);
impl Drop for Job<'_> {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
        self.0.progress.lock().unwrap().busy = false;
    }
}
impl SplitService {
    pub fn new(core: &Engine) -> Result<Self> {
        // Only this feature's disposable objects are removed after an interrupted session.
        let conn = core.conn()?;
        let tables = conn.prepare("SELECT table_name,table_type FROM information_schema.tables WHERE starts_with(table_name,'split_raw_') OR starts_with(table_name,'d_split_') ORDER BY table_type DESC")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<duckdb::Result<Vec<_>>>()?;
        for (name, kind) in tables {
            if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                conn.execute_batch(&format!("DROP {} IF EXISTS {name}", if kind == "VIEW" { "VIEW" } else { "TABLE" }))?;
            }
        }
        let cache = core.root.join("split-imports");
        std::fs::create_dir_all(&cache)?;
        for entry in std::fs::read_dir(&cache)? {
            let path = entry?.path();
            if path.is_file() && path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())) {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(Self::default())
    }
    fn start(&self, phase: &str) -> Result<Job<'_>> {
        if self.busy.swap(true, Ordering::SeqCst) { return Err(AppError::new("BUSY", "正在处理表格，请稍候")); }
        self.cancelled.store(false, Ordering::SeqCst);
        *self.progress.lock().unwrap() = SplitProgress { busy: true, phase: phase.into(), ..Default::default() };
        Ok(Job(self))
    }
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) { Err(AppError::new("CANCELLED", "已停止，已完成的文件保留")) } else { Ok(()) }
    }
    pub fn progress(&self) -> SplitProgress { self.progress.lock().unwrap().clone() }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::SeqCst); }

    pub fn import_file(&self, core: &Engine, path: &str) -> Result<SourceFile> {
        core.guard()?;
        let _gate = core.heavy.lock().unwrap();
        let _job = self.start("正在导入表格")?;
        let path = Path::new(path);
        let size = std::fs::metadata(path)?.len();
        if !path.is_file() || size > 500 * 1024 * 1024 { return Err(AppError::new("IMPORT", "请选择不超过 500MB 的 Excel / CSV 文件")); }
        let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_lowercase();
        if !["xlsx", "xls", "csv"].contains(&extension.as_str()) { return Err(AppError::new("IMPORT", "请选择 .xlsx、.xls 或 .csv 文件")); }
        let file_id = id();
        let cache = core.root.join("split-imports");
        std::fs::create_dir_all(&cache)?;
        let stored = cache.join(format!("{file_id}.{extension}"));
        std::fs::copy(path, &stored)?;
        let conn = core.conn()?;
        let result = (|| {
            conn.execute_batch("BEGIN TRANSACTION")?;
            let mut sheets = Vec::new();
            let encoding = reader::read_each(&stored, &|| self.check(), |sheet| {
                self.progress.lock().unwrap().current = sheet.name.clone();
                if sheet.width == 0 || sheet.rows.is_empty() { return Ok(()); }
                let sheet_id = id();
                let table = format!("split_raw_{sheet_id}");
                create_table(&conn, &table, sheet.width, false)?;
                let padded = crate::appearance::padded_cells(&stored, &sheet.name, sheet.width, sheet.rows.len())?;
                let mut appender = conn.appender(&table)?;
                for (number, row) in sheet.rows.iter().enumerate() {
                    if number % 256 == 0 { self.check()?; }
                    let cells = row.iter().enumerate().map(|(i, cell)| padded_identifier(cell, padded.get(&(number + 1, i + 1)).copied())).collect::<Vec<_>>();
                    append_cells(&mut appender, number + 1, &cells, sheet.width, None)?;
                }
                appender.flush()?;
                drop(appender);
                sheets.push(SourceSheet { id: sheet_id, file_id: file_id.clone(), name: sheet.name, rows: sheet.rows.len(), width: sheet.width,
                    suggested_header_row: reader::suggest_header(&sheet.rows), columns: vec![], read_note: sheet.note });
                Ok(())
            })?;
            self.check()?;
            if sheets.is_empty() { return Err(AppError::new("EMPTY", "文件中没有可拆分的数据工作表")); }
            conn.execute_batch("COMMIT")?;
            Ok(SourceFile { id: file_id, name: path.file_name().unwrap_or_default().to_string_lossy().into(), size, imported_at: now(),
                stored_path: stored.to_string_lossy().into(), encoding, sheets })
        })();
        let file = match result {
            Ok(file) => file,
            Err(error) => { let _ = conn.execute_batch("ROLLBACK"); let _ = std::fs::remove_file(&stored); return Err(error); }
        };
        *self.prepared.lock().unwrap() = None;
        let old = self.imported.lock().unwrap().replace(Imported { file: file.clone(), dataset: None });
        if let Some(old) = old { discard_import(&conn, old); }
        Ok(file)
    }
    pub fn source_page(&self, core: &Engine, sheet_id: &str, offset: usize, limit: usize) -> Result<SplitSourcePage> {
        core.guard()?;
        let imported = self.imported.lock().unwrap();
        let source = imported.as_ref().ok_or_else(|| AppError::new("SOURCE", "请先导入表格"))?;
        let sheet = source.file.sheets.iter().find(|s| s.id == sheet_id).ok_or_else(|| AppError::new("SOURCE", "工作表已变化，请重新选择"))?;
        let rows = read_rows(&core.conn()?, &format!("split_raw_{}", sheet.id), sheet.width, offset, limit.clamp(1, 200))?;
        let columns = (0..sheet.width).map(|i| Column { key: format!("c{i}"), name: reader::cell_address(1, i + 1).trim_end_matches('1').into(), data_type: ColumnType::Auto, suggested_type: ColumnType::Auto }).collect();
        let mut appearance = crate::appearance::read(Path::new(&source.file.stored_path), &sheet.name, sheet.width, offset, limit)
            .unwrap_or_else(|e| SheetAppearance { widths: vec![110.; sheet.width], default_row_height: 32., show_grid_lines: true, note: Some(e.message), ..Default::default() });
        if let Some(note) = &sheet.read_note { appearance.note = Some(note.clone()); }
        Ok(SplitSourcePage { page: DataPage { columns, rows: rows.iter().map(|(_, c)| c.clone()).collect(), row_numbers: rows.iter().map(|(r, _)| *r).collect(), total: sheet.rows, offset, sample: false }, appearance })
    }
    pub fn use_sheet(&self, core: &Engine, sheet_id: &str, range: SheetRange) -> Result<Dataset> {
        core.guard()?;
        let _gate = core.heavy.lock().unwrap();
        let _job = self.start("正在确认数据范围")?;
        let mut imported = self.imported.lock().unwrap();
        let source = imported.as_mut().ok_or_else(|| AppError::new("SOURCE", "请先导入表格"))?;
        let sheet = source.file.sheets.iter().find(|s| s.id == sheet_id).ok_or_else(|| AppError::new("SOURCE", "工作表已变化，请重新选择"))?;
        if range.start_row == 0 || range.end_row < range.start_row || range.end_row > sheet.rows || range.end_row - range.start_row >= 32
            || range.start_column == 0 || range.end_column < range.start_column || range.end_column > sheet.width {
            return Err(AppError::new("HEADER", "请框选工作表内最多 32 行的表头区域"));
        }
        let conn = core.conn()?;
        let table = format!("split_raw_{}", sheet.id);
        let mut headers = read_rows(&conn, &table, sheet.width, range.start_row - 1, range.end_row - range.start_row + 101)?;
        let look = crate::appearance::read(Path::new(&source.file.stored_path), &sheet.name, sheet.width, range.start_row - 1, range.end_row - range.start_row + 1)?;
        for merge in &look.merges {
            if merge.start_row < range.start_row && merge.end_row >= range.start_row && !headers.iter().any(|(r, _)| *r == merge.start_row) {
                headers.extend(read_rows(&conn, &table, sheet.width, merge.start_row - 1, 1)?);
            }
        }
        let labels = (range.start_column..=range.end_column).map(|column| {
            let mut parts = Vec::new();
            for row in range.start_row..=range.end_row {
                let (r, c) = look.merges.iter().find(|m| row >= m.start_row && row <= m.end_row && column >= m.start_column && column <= m.end_column)
                    .map(|m| (m.start_row, m.start_column)).unwrap_or((row, column));
                let label = headers.iter().find(|(n, _)| *n == r).and_then(|(_, v)| v.get(c - 1)).map(|v| v.text().trim().to_string()).unwrap_or_default();
                if !label.is_empty() && parts.last() != Some(&label) { parts.push(label); }
            }
            Cell::Text(parts.join(" / "))
        }).collect::<Vec<_>>();
        let columns = reader::header_names(&labels, labels.len()).into_iter().enumerate().map(|(i, name)| {
            let values = headers.iter().filter(|(r, _)| *r > range.end_row).filter_map(|(_, v)| v.get(range.start_column + i - 1).cloned()).collect::<Vec<_>>();
            Column { key: reader::business_key(&name), suggested_type: reader::suggest_column_type(&name, &values), name, data_type: ColumnType::Auto }
        }).collect::<Vec<_>>();
        let dataset_id = format!("split_{}", id());
        let selection = (range.start_column - 1..range.end_column).enumerate().map(|(i, j)| format!("c{j} AS c{i},t{j} AS t{i},CASE WHEN t{j}=2 THEN TRY_CAST(c{j} AS DOUBLE) END AS n{i}")).collect::<Vec<_>>().join(",");
        conn.execute_batch(&format!("CREATE VIEW d_{dataset_id} AS SELECT _row,{selection} FROM {table} WHERE _row>{}", range.end_row))?;
        let count = sheet.rows.saturating_sub(range.end_row);
        let dataset = Dataset { id: dataset_id, group_id: None, name: sheet.name.clone(), created_at: now(), columns,
            summary: Summary { input_rows: count, output_rows: count, ..Default::default() }, sample: false, rules: RuleSet::default(), exports: vec![] };
        *self.prepared.lock().unwrap() = None;
        if let Some(old) = source.dataset.replace(dataset.clone()) { let _ = conn.execute_batch(&format!("DROP VIEW IF EXISTS d_{}", old.id)); }
        Ok(dataset)
    }
    fn dataset(&self, core: &Engine, dataset_id: &str) -> Result<Dataset> {
        if let Some(ds) = self.imported.lock().unwrap().as_ref().and_then(|s| s.dataset.as_ref()).filter(|d| d.id == dataset_id) { return Ok(ds.clone()); }
        let ds = core.dataset(dataset_id)?;
        if ds.sample { return Err(AppError::new("SOURCE", "请选择完整汇总结果，样本预览不能用于拆分导出")); }
        Ok(ds)
    }
    pub fn prepare(&self, core: &Engine, request: SplitRequest, edits: Vec<SplitEdit>, output: SplitOutput) -> Result<SplitPlan> {
        core.guard()?;
        let _gate = core.heavy.lock().unwrap();
        let _job = self.start("正在核对拆分结果")?;
        let dataset = self.dataset(core, &request.query.dataset_id)?;
        if request.format != "xlsx" && request.format != "csv" { return Err(AppError::new("FORMAT", "请选择 XLSX 或 CSV")); }
        if request.naming.is_empty() || request.naming.len() > 64 || request.separator.len() > 32 { return Err(AppError::new("NAME", "请添加 1～64 项文件名内容，分隔符不超过 32 字节")); }
        let field = index(&dataset, &request.field)?;
        let indices = request.columns.iter().map(|key| index(&dataset, key)).collect::<Result<Vec<_>>>()?;
        if indices.is_empty() || indices.iter().copied().collect::<HashSet<_>>().len() != indices.len() { return Err(AppError::new("COLUMN", "请至少保留一列，且不能重复选择同一列")); }
        let grouping_key = serde_json::to_string(&(&dataset.id, &dataset.created_at, &request.field, &request.query.filters, &request.query.search, &request.query.sorts))?;
        let previous = self.prepared.lock().unwrap().take();
        let (conn, groups) = if let Some(previous) = previous.filter(|p| p.grouping_key == grouping_key) {
            // Naming, column selection and output changes reuse this immutable membership snapshot.
            (previous.conn, previous.members)
        } else {
            let conn = core.conn()?;
            let (condition, args) = export_conditions(&dataset, &request.query)?;
            let kind = format!("CASE WHEN COALESCE(c{field},'')='' THEN 0 ELSE t{field} END");
            conn.execute(&format!("CREATE TEMP TABLE split_order AS SELECT _row,ROW_NUMBER() OVER ({}) AS ordinal,COALESCE(c{field},'') AS value,{kind} AS kind FROM d_{}{condition} ORDER BY value,kind,ordinal", order(&dataset, &request.query)?, dataset.id), params_from_iter(args))?;
            self.check()?;
            let groups = conn.prepare("SELECT value,kind,COUNT(*) FROM split_order GROUP BY value,kind ORDER BY MIN(ordinal)")?
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u8>(1)?, r.get::<_, i64>(2)? as usize)))?
                .collect::<duckdb::Result<Vec<_>>>()?;
            (conn, groups)
        };
        let mut extras = HashMap::new();
        for part in &request.naming {
            if let SplitNamePart::Column { key } = part {
                if extras.contains_key(key) { continue; }
                let i = index(&dataset, key)?;
                let values = conn.prepare(&format!("SELECT o.value,o.kind,COUNT(DISTINCT (COALESCE(d.c{i},''),CASE WHEN COALESCE(d.c{i},'')='' THEN 0 ELSE d.t{i} END)),MIN(COALESCE(d.c{i},'')) FROM split_order o JOIN d_{} d USING(_row) GROUP BY o.value,o.kind", dataset.id))?
                    .query_map([], |r| Ok(((r.get::<_, String>(0)?, r.get::<_, u8>(1)?), (r.get::<_, i64>(2)?, r.get::<_, String>(3)?))))?
                    .collect::<duckdb::Result<HashMap<_, _>>>()?;
                extras.insert(key.clone(), values);
            }
        }
        let (directory, create_directory) = output_directory(&output)?;
        let mut used = HashSet::new();
        if let Some(dir) = directory.as_ref().filter(|d| d.is_dir()) {
            for entry in std::fs::read_dir(dir)? { used.insert(entry?.file_name().to_string_lossy().to_lowercase()); }
        }
        let budget = filename_budget(directory.as_deref(), &request.format)?;
        let edits = edits.into_iter().map(|e| (e.id.clone(), e)).collect::<HashMap<_, _>>();
        let mut rows = Vec::with_capacity(groups.len());
        for (number, (value, kind, count)) in groups.iter().cloned().enumerate() {
            self.check()?;
            let group_id = serde_json::to_string(&(kind, &value))?;
            let edit = edits.get(&group_id);
            let selected = edit.is_none_or(|e| e.selected);
            let custom = edit.and_then(|e| e.name.as_ref());
            let mut error = None;
            let name = if let Some(custom) = custom {
                if custom.trim().is_empty() { error = Some("文件名不能为空，请输入名称或恢复规则命名".into()); }
                custom.clone()
            } else {
                let mut parts = Vec::new();
                for part in &request.naming {
                    parts.push(match part {
                        SplitNamePart::Value => if kind == 0 { "未填写".into() } else { value.clone() },
                        SplitNamePart::Text { text } => text.clone(),
                        SplitNamePart::Source => request.source_name.clone(),
                        SplitNamePart::Sheet => request.sheet_name.clone(),
                        SplitNamePart::Date => request.date.clone(),
                        SplitNamePart::Sequence => format!("{:03}", number + 1),
                        SplitNamePart::Column { key } => {
                            let v = extras.get(key).and_then(|map| map.get(&(value.clone(), kind)));
                            if let Some((1, text)) = v { text.clone() } else {
                                error = Some(format!("命名字段“{}”在本组存在多个值，请调整命名规则或单独改名", dataset.columns[index(&dataset, key)?].name)); String::new()
                            }
                        }
                    });
                }
                parts.join(&request.separator)
            };
            if name.trim().is_empty() && error.is_none() { error = Some("文件名内容为空，请添加分类值或固定文字".into()); }
            let (base, changed) = safe_stem(&name, budget);
            let mut candidate = format!("{base}.{}", request.format);
            let mut suffix = 2;
            while selected && used.contains(&candidate.to_lowercase()) {
                let tail = format!(" ({suffix})");
                candidate = format!("{}{}.{}", truncate_utf16(&base, budget.saturating_sub(tail.encode_utf16().count())), tail, request.format);
                suffix += 1;
            }
            if selected { used.insert(candidate.to_lowercase()); }
            let mut notes = Vec::new();
            if changed { notes.push("名称已规范或缩短"); }
            if suffix > 2 { notes.push("重名已自动编号"); }
            if request.format == "xlsx" && count > 1_048_575 { error = Some("本组超过 Excel 行数上限，请改为 CSV".into()); }
            rows.push(SplitGroup { id: group_id, value, kind, rows: count, selected, filename: candidate, note: notes.join("；"), error,
                state: "ready".into(), path: None });
        }
        let plan = SplitPlan { id: id(), total_rows: rows.iter().map(|g| g.rows).sum(), selected_rows: rows.iter().filter(|g| g.selected).map(|g| g.rows).sum(),
            groups: rows, directory: directory.as_ref().map(|p| p.to_string_lossy().into()), note: None };
        *self.prepared.lock().unwrap() = Some(Prepared { plan: plan.clone(), conn, dataset, indices, grouping_key, members: groups, create_directory, started: false });
        Ok(plan)
    }
    pub fn page(&self, core: &Engine, plan_id: &str, group_id: Option<&str>, offset: usize, limit: usize, sorts: Vec<Sort>) -> Result<DataPage> {
        core.guard()?;
        let prepared = self.prepared.lock().unwrap();
        let p = prepared.as_ref().filter(|p| p.plan.id == plan_id).ok_or_else(stale)?;
        let group = group_id.map(|id| p.plan.groups.iter().find(|g| g.id == id).ok_or_else(stale)).transpose()?;
        let columns = p.indices.iter().map(|i| p.dataset.columns[*i].clone()).collect::<Vec<_>>();
        let selection = p.indices.iter().flat_map(|i| [format!("c{i}"), format!("t{i}")]).collect::<Vec<_>>().join(",");
        let condition = if group.is_some() { " WHERE o.value=? AND o.kind=?" } else { "" };
        let mut args = group.map(|g| vec![duckdb::types::Value::Text(g.value.clone()), duckdb::types::Value::UTinyInt(g.kind)]).unwrap_or_default();
        let ordering = if sorts.is_empty() { " ORDER BY o.ordinal".to_string() } else { order(&p.dataset, &DatasetQuery { sorts, ..Default::default() })? };
        args.extend([duckdb::types::Value::BigInt(limit.clamp(1, 1000) as i64), duckdb::types::Value::BigInt(offset as i64)]);
        let mut statement = p.conn.prepare(&format!("SELECT d._row,{selection} FROM d_{} d JOIN split_order o USING(_row){condition}{ordering} LIMIT ? OFFSET ?", p.dataset.id))?;
        let mut cursor = statement.query(params_from_iter(args))?;
        let mut rows = Vec::new(); let mut row_numbers = Vec::new();
        while let Some(row) = cursor.next()? {
            row_numbers.push(row.get::<_, i64>(0)? as usize);
            rows.push((0..columns.len()).map(|i| cell_from_row(row, 1 + i * 2)).collect::<Result<_>>()?);
        }
        Ok(DataPage { columns, rows, row_numbers, total: group.map(|g| g.rows).unwrap_or(p.plan.total_rows), offset, sample: false })
    }
    pub fn execute(&self, core: &Engine, plan_id: &str) -> Result<SplitPlan> {
        core.guard()?;
        let _gate = core.heavy.lock().unwrap();
        let _job = self.start("正在导出拆分文件")?;
        let mut prepared = self.prepared.lock().unwrap();
        let p = prepared.as_mut().filter(|p| p.plan.id == plan_id).ok_or_else(stale)?;
        self.dataset(core, &p.dataset.id)?;
        let directory = PathBuf::from(p.plan.directory.as_ref().ok_or_else(|| AppError::new("DIRECTORY", "请选择保存文件夹"))?);
        if !p.plan.groups.iter().any(|g| g.selected && g.state != "saved") { return Err(AppError::new("SELECT", "请勾选需要导出的分类")); }
        if p.plan.groups.iter().any(|g| g.selected && g.error.is_some()) { return Err(AppError::new("NAME", "请先处理所选文件的命名问题")); }
        if !p.started && p.create_directory { std::fs::create_dir(&directory).map_err(|e| AppError::new("DIRECTORY", format!("导出目录无法创建或已发生变化，请重新核对：{e}")))?; }
        if !directory.is_dir() { return Err(AppError::new("DIRECTORY", "导出目录已被移动或删除，请重新选择")); }
        p.started = true;
        self.progress.lock().unwrap().total = p.plan.groups.iter().filter(|g| g.selected && g.state != "saved").count();
        let conn = core.conn()?;
        let columns = p.indices.iter().map(|i| p.dataset.columns[*i].clone()).collect::<Vec<_>>();
        for group in p.plan.groups.iter_mut().filter(|g| g.selected && g.state != "saved") {
            if self.check().is_err() { group.state = "cancelled".into(); continue; }
            self.progress.lock().unwrap().current = group.filename.clone();
            let path = directory.join(&group.filename);
            let result = (|| {
                let temp = temporary(&path)?;
                let result = (|| {
                    let mut statement = p.conn.prepare("SELECT _row FROM split_order WHERE value=? AND kind=? ORDER BY ordinal")?;
                    let cursor = statement.stream_arrow(params![&group.value, group.kind])?;
                    let mut next = export_rows(&conn, cursor, &p.dataset.id, &p.indices);
                    let mut written = 0;
                    write_rows(&temp, &path, &columns, || {
                        if let Err(error) = self.check() { return Some(Err(error)); }
                        let row = next(); if matches!(&row, Some(Ok(_))) { written += 1; } row
                    })?;
                    self.check()?;
                    if written != group.rows { return Err(AppError::new("INCOMPLETE", "数据行数已变化，请重新核对拆分结果")); }
                    crate::rename::move_no_replace(&temp, &path).map_err(|e| AppError::new("EXPORT", format!("未能保存，同名文件不会覆盖：{e}")))?;
                    Ok(())
                })();
                if temp.exists() { let _ = std::fs::remove_file(&temp); }
                result
            })();
            match result {
                Ok(()) => { group.state = "saved".into(); group.path = Some(path.to_string_lossy().into()); group.note = "已保存".into(); }
                Err(error) => { group.state = if error.code == "CANCELLED" { "cancelled" } else { "failed" }.into(); group.note = error.message; }
            }
            self.progress.lock().unwrap().done += 1;
        }
        if p.plan.groups.iter().any(|g| g.state == "saved") {
            if let Err(e) = core.record_export(&p.dataset.id, &directory) { p.plan.note = Some(format!("文件已保存，导出记录未能更新：{}", e.message)); }
        }
        self.progress.lock().unwrap().phase = if self.cancelled.load(Ordering::SeqCst) { "已停止导出" } else { "导出结束" }.into();
        Ok(p.plan.clone())
    }
    pub fn allows_path(&self, path: &Path) -> bool {
        self.prepared.lock().unwrap().as_ref().is_some_and(|p| p.plan.groups.iter().any(|g| g.state == "saved") && p.plan.directory.as_ref().is_some_and(|d| Path::new(d) == path))
    }
}
fn stale() -> AppError { AppError::new("STALE", "拆分预览已更新，请重新核对后导出") }
fn discard_import(conn: &Connection, old: Imported) {
    if let Some(ds) = old.dataset { let _ = conn.execute_batch(&format!("DROP VIEW IF EXISTS d_{}", ds.id)); }
    for sheet in old.file.sheets { let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS split_raw_{}", sheet.id)); }
    let _ = std::fs::remove_file(old.file.stored_path);
}
fn truncate_utf16(text: &str, max: usize) -> String {
    let mut size = 0;
    text.chars().take_while(|c| { size += c.len_utf16(); size <= max }).collect()
}
fn safe_stem(name: &str, budget: usize) -> (String, bool) {
    let mut stem = name.trim().to_string();
    if stem.to_lowercase().ends_with(".xlsx") { stem.truncate(stem.len() - 5); }
    else if stem.to_lowercase().ends_with(".csv") { stem.truncate(stem.len() - 4); }
    stem = stem.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect::<String>().trim().trim_end_matches('.').to_string();
    if stem.is_empty() { stem = "未命名".into(); }
    let device = stem.split('.').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"].contains(&device.as_str())
        || ["COM", "LPT"].iter().any(|prefix| device.strip_prefix(prefix).is_some_and(|tail| tail.chars().count() == 1 && "123456789¹²³".contains(tail))) { stem.insert(0, '_'); }
    stem = truncate_utf16(&stem, budget).trim_end_matches([' ', '.']).to_string();
    let changed = stem != name;
    (stem, changed)
}
fn filename_budget(directory: Option<&Path>, extension: &str) -> Result<usize> {
    let budget = directory.map(|d| 248usize.saturating_sub(d.to_string_lossy().encode_utf16().count() + extension.len() + 2)).unwrap_or(180).min(180);
    if budget < 16 { return Err(AppError::new("PATH", "目标路径过长，请选择层级更浅的文件夹")); }
    Ok(budget)
}
fn output_directory(output: &SplitOutput) -> Result<(Option<PathBuf>, bool)> {
    if output.directory.trim().is_empty() { return Ok((None, false)); }
    let parent = std::fs::canonicalize(&output.directory).map_err(|e| AppError::new("DIRECTORY", format!("保存文件夹不存在或无法访问：{e}")))?;
    if !parent.is_dir() { return Err(AppError::new("DIRECTORY", "请选择保存文件夹")); }
    if !output.new_folder { return Ok((Some(parent), false)); }
    let (stem, _) = safe_stem(&output.folder_name, 100);
    let mut path = parent.join(&stem); let mut number = 2;
    while path.exists() { path = parent.join(format!("{stem} ({number})")); number += 1; }
    Ok((Some(path), true))
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
