//! Independent, read-only planning followed by journalled, non-overwriting renames.
use crate::{model::*, reader};
use calamine::{open_workbook_auto, Data, Reader, Sheets};
use crate::rename_read::{Scope, SheetCells};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs::{self, File, FileTimes, OpenOptions}, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::UNIX_EPOCH};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameCell { pub sheet: String, pub row: usize, pub column: usize }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameArea { pub sheet: String, pub range: SheetRange }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameCondition { pub source: RenameCell, pub operator: String, pub value: String }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RenameLookupEntry { pub key: String, pub value: String }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RenamePart {
    Cell { source: RenameCell, decimals: Option<u8> },
    Text { text: String },
    Lookup { source: RenameCell, #[serde(default)] name: String, entries: Vec<RenameLookupEntry>, fallback: Option<String> },
    Condition { condition: RenameCondition, text: String },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameSum { pub area: RenameArea, pub divisor: f64, pub min_filled: usize, pub condition: Option<RenameCondition> }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameCheck { pub label: String, pub expected: RenameCell, pub sums: Vec<RenameSum>, pub tolerance_percent: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameRules { pub parts: Vec<RenamePart>, pub separator: String, pub prefix: String, pub suffix: String, pub checks: Vec<RenameCheck> }
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all="camelCase")]
pub enum RenameOutputMode { #[default] Original, Copy, Move }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all="camelCase")]
pub struct RenameOutput { pub mode:RenameOutputMode, pub directory:Option<String>, #[serde(default="default_numbering")] pub number_conflicts:bool }
fn default_numbering()->bool {true}
impl Default for RenameOutput { fn default()->Self {Self {mode:RenameOutputMode::Original,directory:None,number_conflicts:true}} }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameFile { pub id: String, pub path: String, pub name: String }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RenameScan { pub files: Vec<RenameFile>, pub warnings: Vec<String> }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenamePage { pub sheet: SourceSheet, pub page: DataPage, pub appearance: SheetAppearance, pub column_offset: usize, pub total_columns: usize }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameRow {
    pub id: String, pub path: String, pub original_name: String, pub new_name: String,
    pub values: Vec<String>, pub checks: Vec<String>, pub state: String, pub message: String,
    #[serde(default)] pub has_mismatch: bool,
    #[serde(default)] pub target_path:Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlan { pub id: String, pub rows: Vec<RenameRow>, pub journal_path: Option<String>, #[serde(default)] pub value_labels: Vec<String>, #[serde(default)] pub output:RenameOutput }
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct RenameProgress { pub busy: bool, pub phase: String, pub done: usize, pub total: usize, pub current: String }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stamp { size: u64, modified: u128 }
fn stamp(path: &Path) -> Result<Stamp> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() { return Err(AppError::new("FILE", "仅处理普通文件，不处理链接或文件夹")); }
    Ok(Stamp { size: meta.len(), modified: meta.modified()?.duration_since(UNIX_EPOCH).map_err(|e| AppError::new("FILE", e.to_string()))?.as_nanos() })
}
struct Book { path: String, stamp: Stamp, workbook: Sheets<BufReader<File>>, scopes: HashMap<String,Scope>, sheets: HashMap<String,(Scope,SheetCells)> }
impl Book {
    fn open(path: &str) -> Result<Self> {
        let mark = stamp(Path::new(path))?;
        if mark.size > 100 * 1024 * 1024 { return Err(AppError::new("LIMIT", "单文件超过 100MB，请拆分后处理")); }
        let workbook = open_workbook_auto(path).map_err(|e| AppError::new("EXCEL", format!("无法读取 Excel，可能已损坏、加密或被占用：{e}")))?;
        Ok(Self { path: path.into(), stamp: mark, workbook, scopes: HashMap::new(), sheets: HashMap::new() })
    }
    fn configure(&mut self, rules: &RenameRules) {
        let mut scopes = HashMap::<String,Scope>::new();
        fn cell(scopes: &mut HashMap<String,Scope>, c: &RenameCell) { scopes.entry(c.sheet.clone()).or_default().areas.push(SheetRange {start_row:c.row,end_row:c.row,start_column:c.column,end_column:c.column}); }
        for part in &rules.parts {match part {RenamePart::Cell {source,..}|RenamePart::Lookup {source,..}=>cell(&mut scopes,source),RenamePart::Condition {condition,..}=>cell(&mut scopes,&condition.source),RenamePart::Text {..}=>{}}}
        for check in &rules.checks {
            cell(&mut scopes,&check.expected);
            for sum in &check.sums {
                let scope=scopes.entry(sum.area.sheet.clone()).or_default();scope.areas.push(sum.area.range.clone());
                if sum.min_filled>0 {scope.filled_rows.push((sum.area.range.start_row,sum.area.range.end_row));}
                if let Some(condition)=&sum.condition {cell(&mut scopes,&condition.source);}
            }
        }
        for scope in scopes.values_mut() {scope.areas.sort_by_key(|r|(r.start_row,r.start_column,r.end_row,r.end_column));scope.areas.dedup();scope.filled_rows.sort();scope.filled_rows.dedup();}
        self.sheets.retain(|name,(scope,_)|scopes.get(name)==Some(scope));self.scopes=scopes;
    }
    fn sheet(&mut self, name: &str) -> Result<&SheetCells> {
        if !self.sheets.contains_key(name) {
            let scope=self.scopes.get(name).cloned().unwrap_or_default();
            let cached_cells:usize=self.sheets.values().map(|(_,s)|s.cells.len()).sum();
            let data=crate::rename_read::read(&mut self.workbook,Path::new(&self.path),name,&scope,20_000_000usize.saturating_sub(cached_cells))?;
            self.sheets.insert(name.into(),(scope,data));
        }
        Ok(&self.sheets[name].1)
    }
    fn cell(&mut self, source: &RenameCell) -> Result<Cell> {
        validate_cell(source)?;
        let s = self.sheet(&source.sheet)?;
        checked_cell(s, source.row, source.column)
    }
    fn condition(&mut self, condition: &RenameCondition) -> Result<bool> {
        let cell = self.cell(&condition.source)?;
        match condition.operator.as_str() {
            "positive" => Ok(numeric(&cell)? > 0.),
            "nonempty" => Ok(!cell.text().trim().is_empty()),
            "equals" => Ok(cell.text().trim() == condition.value.trim()),
            _ => Err(AppError::new("RULE", "条件仅支持大于零、有内容或等于指定值")),
        }
    }
    fn sum(&mut self, sum: &RenameSum) -> Result<f64> {
        if let Some(c) = &sum.condition { if !self.condition(c)? { return Ok(0.); } }
        let s = self.sheet(&sum.area.sheet)?;
        let r = &sum.area.range;
        let mut total = 0.;
        for (&(row,col),_) in s.cells.range((r.start_row,0)..=(r.end_row,usize::MAX)) {
            if col<r.start_column || col>r.end_column || (sum.min_filled>0 && s.filled.get(&row).copied().unwrap_or(0)<sum.min_filled) {continue;}
            let value = checked_cell(s, row, col)?;
            if value == Cell::Empty || value.text().trim().is_empty() { continue; }
            total += numeric(&value).map_err(|_| AppError::new("NUMBER", format!("{}!{} 不是可计算的数字", sum.area.sheet, reader::cell_address(row, col))))?;
        }
        let value = total / sum.divisor;
        if !value.is_finite() { return Err(AppError::new("NUMBER", "合计超出可计算范围")); }
        Ok(value)
    }
}
fn to_cell(value: &Data) -> Cell {
    match value {
        Data::Empty => Cell::Empty, Data::String(v) | Data::DurationIso(v) => Cell::Text(v.clone()),
        Data::Int(v) => Cell::Number(*v as f64), Data::Float(v) => Cell::Number(*v), Data::Bool(v) => Cell::Bool(*v),
        Data::DateTime(v) => v.as_datetime().map(|d| Cell::Date(d.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or(Cell::Number(v.as_f64())),
        Data::DateTimeIso(v) => Cell::Date(v.clone()), Data::Error(v) => Cell::Text(format!("#Excel错误:{v:?}")),
    }
}
fn checked_cell(sheet: &SheetCells, row: usize, column: usize) -> Result<Cell> {
    let cell = sheet.cells.get(&(row,column));
    let data = cell.map(|c|&c.value).unwrap_or(&Data::Empty);
    if matches!(data, Data::Error(_)) { return Err(AppError::new("CELL", format!("{} 包含 Excel 错误", reader::cell_address(row, column)))); }
    if matches!(data, Data::Empty) && cell.is_some_and(|c|c.formula) {
        return Err(AppError::new("FORMULA", format!("{} 的公式没有缓存值，请在 Excel 中重新计算并保存", reader::cell_address(row, column))));
    }
    Ok(to_cell(data))
}
fn numeric(cell: &Cell) -> Result<f64> {
    let n = match cell { Cell::Number(v) => Some(*v), Cell::Text(v) => reader::parse_number(v), _ => None };
    n.filter(|v| v.is_finite()).ok_or_else(|| AppError::new("NUMBER", "单元格为空或不是有效数字"))
}
fn validate_cell(cell: &RenameCell) -> Result<()> {
    if cell.sheet.is_empty() || cell.row == 0 || cell.row > 1_048_576 || cell.column == 0 || cell.column > 16384 { return Err(AppError::new("RULE", "请选择有效的工作表和单元格")); }
    Ok(())
}
fn validate_rules(rules: &RenameRules) -> Result<()> {
    if rules.parts.is_empty() || rules.parts.len() > 64 || rules.checks.len() > 32 { return Err(AppError::new("RULE", "请添加 1～64 项命名内容，校验最多 32 项")); }
    for part in &rules.parts {
        match part {
            RenamePart::Cell { source, decimals } => { validate_cell(source)?; if decimals.is_some_and(|v| v > 10) { return Err(AppError::new("RULE", "小数位数应为 0～10")); } },
            RenamePart::Lookup { source, entries, .. } => {
                validate_cell(source)?;
                if entries.is_empty() || entries.len() > 10000 { return Err(AppError::new("RULE", "对照表需要 1～10000 行")); }
                let mut keys = HashSet::new();
                for e in entries { if e.key.trim().is_empty() || e.value.trim().is_empty() || !keys.insert(e.key.trim()) { return Err(AppError::new("RULE", "对照表包含空值或重复编码，请先修正")); } }
            },
            RenamePart::Condition { condition, .. } => validate_cell(&condition.source)?,
            RenamePart::Text { .. } => {},
        }
    }
    for check in &rules.checks {
        validate_cell(&check.expected)?;
        if check.label.trim().is_empty() || check.sums.is_empty() || check.sums.len() > 32 || !check.tolerance_percent.is_finite() || !(0. ..=100.).contains(&check.tolerance_percent) { return Err(AppError::new("RULE", "校验需要异常标签、合计区域和 0～100% 的容差")); }
        for sum in &check.sums {
            let r = &sum.area.range;
            validate_cell(&RenameCell { sheet: sum.area.sheet.clone(), row: r.start_row, column: r.start_column })?;
            if r.end_row < r.start_row || r.end_column < r.start_column || r.end_row > 1_048_576 || r.end_column > 16384 || (r.end_row - r.start_row + 1).saturating_mul(r.end_column - r.start_column + 1) > 20_000_000 || !sum.divisor.is_finite() || sum.divisor <= 0. || sum.min_filled > 16384 { return Err(AppError::new("RULE", "合计区域或单位除数无效")); }
            if let Some(c) = &sum.condition { validate_cell(&c.source)?; }
        }
    }
    Ok(())
}
fn safe_name(stem: &str, extension: &str) -> Result<(String, bool)> {
    let clean: String = stem.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']);
    if clean.is_empty() { return Err(AppError::new("NAME", "生成的文件名为空")); }
    let base = clean.split('.').next().unwrap_or("").trim_end().to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"].contains(&base.as_str()) || (base.starts_with("COM") || base.starts_with("LPT")) && matches!(&base[3..], "1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"|"¹"|"²"|"³") { return Err(AppError::new("NAME", "文件名是 Windows 保留名称，请添加前缀")); }
    let name = format!("{clean}.{extension}");
    if name.encode_utf16().count() > 240 { return Err(AppError::new("NAME", "文件名过长，请减少命名内容")); }
    Ok((name, clean != stem))
}
fn lookup_tables(rules: &RenameRules) -> Vec<HashMap<&str, &str>> {
    rules.parts.iter().map(|p| match p { RenamePart::Lookup { entries, .. } => entries.iter().map(|e| (e.key.trim(), e.value.trim())).collect(), _ => HashMap::new() }).collect()
}
fn row_for(file: &RenameFile, rules: &RenameRules, lookups: &[HashMap<&str, &str>], book: &mut Book, output:&RenameOutput) -> Result<RenameRow> {
    book.configure(rules);
    let mut values = Vec::new();
    for (index, part) in rules.parts.iter().enumerate() {
        let value = match part {
            RenamePart::Cell { source, decimals } => {
                let c = book.cell(source)?;
                if c.text().trim().is_empty() { return Err(AppError::new("EMPTY", format!("{}!{} 为空，已跳过", source.sheet, reader::cell_address(source.row, source.column)))); }
                if let Some(d) = decimals { format!("{:.*}", *d as usize, numeric(&c)?) } else { c.text().trim().into() }
            },
            RenamePart::Text { text } => text.clone(),
            RenamePart::Lookup { source, fallback, .. } => {
                let key = book.cell(source)?.text();
                lookups[index].get(key.trim()).map(|v| (*v).to_owned()).or_else(|| fallback.clone().filter(|s| !s.trim().is_empty()))
                    .ok_or_else(|| AppError::new("LOOKUP", format!("对照表找不到“{}”，请补充对照信息", key.trim())))?
            },
            RenamePart::Condition { condition, text } => if book.condition(condition)? { text.clone() } else { String::new() },
        };
        values.push(value);
    }
    let mut checks = Vec::new();
    let mut labels = Vec::new();
    for check in &rules.checks {
        let expected = numeric(&book.cell(&check.expected)?)?;
        let mut actual = 0.;
        for sum in &check.sums { actual += book.sum(sum)?; }
        if !actual.is_finite() { return Err(AppError::new("NUMBER", "合计超出可计算范围")); }
        let difference = (actual - expected).abs();
        let matches = if expected == 0. { difference <= 1e-10 } else { difference <= expected.abs() * check.tolerance_percent / 100. + 1e-10 };
        checks.push(format!("{}：{}（合计 {:.2} / 填写 {:.2}，容差 {:.2}%）", check.label, if matches { "通过" } else { "不一致" }, actual, expected, check.tolerance_percent));
        if !matches { labels.push(check.label.clone()); }
    }
    let content = values.iter().filter(|v| !v.is_empty()).cloned().collect::<Vec<_>>().join(&rules.separator);
    let tags = if labels.is_empty() { String::new() } else { format!("{}_", labels.join("_")) };
    let stem = format!("{tags}{}{content}{}", rules.prefix, rules.suffix);
    let ext = Path::new(&file.path).extension().and_then(|v| v.to_str()).unwrap_or("xlsx");
    let (new_name, cleaned) = safe_name(&stem, ext)?;
    let mut row = RenameRow { id: file.id.clone(), path: file.path.clone(), original_name: file.name.clone(), new_name, values, checks, has_mismatch: !labels.is_empty(), target_path:None, state: "ready".into(), message: if cleaned { "文件名中的非法字符已替换，结尾空格/句点已去除".into() } else { String::new() } };
    let target=output.directory.as_ref().map(|dir|Path::new(dir).join(&row.new_name)).unwrap_or_else(||Path::new(&file.path).with_file_name(&row.new_name));
    if output.mode != RenameOutputMode::Original {row.target_path=Some(target.to_string_lossy().into());}
    if target.to_string_lossy().encode_utf16().count() >= 260 { row.state = "skipped".into(); row.message = "完整路径过长，请缩短文件名或移动至较短路径".into(); }
    else if path_key(&target)==path_key(Path::new(&file.path)) { row.state = "unchanged".into(); row.message = "已在目标位置且名称未变化".into(); }
    else if (output.mode == RenameOutputMode::Original || !output.number_conflicts) && target.try_exists()? { row.state = "skipped".into(); row.message = "目标名称已存在，保留原文件".into(); }
    Ok(row)
}
fn failed_row(file: &RenameFile, error: AppError) -> RenameRow {
    RenameRow { id: file.id.clone(), path: file.path.clone(), original_name: file.name.clone(), new_name: String::new(), values: vec![], checks: vec![], has_mismatch: false, target_path:None, state: "skipped".into(), message: error.message }
}
#[derive(Clone, Serialize, Deserialize)]
struct SavedPlan { plan: RenamePlan, stamps: HashMap<String, Stamp>, #[serde(default)] target_stamps:HashMap<String,Stamp>, #[serde(default)] restored_stamps:HashMap<String,Stamp> }
#[derive(Serialize, Deserialize)]
struct JournalEvent { id: String, state: String, message: String, #[serde(default)] target_stamp:Option<Stamp>, #[serde(default)] restored_stamp:Option<Stamp> }
#[derive(Default)]
pub struct RenameService {
    files: Mutex<Vec<RenameFile>>, book: Mutex<Option<Book>>, plan: Mutex<Option<SavedPlan>>,
    busy: AtomicBool, cancelled: AtomicBool, progress: Mutex<RenameProgress>,
}
struct Job<'a>(&'a RenameService);
impl Drop for Job<'_> { fn drop(&mut self) { self.0.busy.store(false, Ordering::SeqCst); } }
impl RenameService {
    pub fn files(&self) -> Vec<RenameFile> { self.files.lock().unwrap().clone() }
    fn sync_file(&self, row: &RenameRow) {
        let target = target_path(row).to_string_lossy().into_owned();
        let mut files = self.files.lock().unwrap();
        if let Some(f) = files.iter_mut().find(|f| f.id == row.id || f.path == row.path || f.path == target) {
            if matches!(row.state.as_str(),"renamed"|"moved") { f.path = target; f.name = row.new_name.clone(); }
            else if row.state == "restored" { f.path = row.path.clone(); f.name = row.original_name.clone(); }
        }
    }
    fn begin(&self, phase: &str, total: usize) -> Result<Job<'_>> {
        self.busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).map_err(|_| AppError::new("BUSY", "重命名任务正在处理中，请稍候"))?;
        self.cancelled.store(false, Ordering::SeqCst);
        *self.progress.lock().unwrap() = RenameProgress { busy: true, phase: phase.into(), done: 0, total, current: String::new() };
        Ok(Job(self))
    }
    pub fn progress(&self) -> RenameProgress { let mut p = self.progress.lock().unwrap().clone(); p.busy = self.busy.load(Ordering::SeqCst); p }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::SeqCst); }
    fn file(&self, file_id: &str) -> Result<RenameFile> { self.files.lock().unwrap().iter().find(|f| f.id == file_id).cloned().ok_or_else(|| AppError::new("FILE", "文件已移出清单，请重新添加")) }
    pub fn scan(&self, paths: Vec<String>, recursive: bool) -> Result<RenameScan> {
        let _job = self.begin("扫描文件", 0)?;
        let existing: HashMap<String, String> = self.files.lock().unwrap().iter().map(|f| (f.path.to_lowercase(), f.id.clone())).collect();
        let mut files = Vec::new(); let mut warnings = Vec::new(); let mut seen = HashSet::new(); let mut directories = HashSet::new();
        let mut pending: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        while let Some(path) = pending.pop() {
            if self.cancelled.load(Ordering::SeqCst) { return Err(AppError::new("CANCELLED", "已停止扫描，原清单保持不变")); }
            let meta = match fs::symlink_metadata(&path) { Ok(v) => v, Err(e) => { warnings.push(format!("{}：{e}", path.display())); continue; } };
            if meta.file_type().is_symlink() { continue; }
            if meta.is_dir() {
                let directory = fs::canonicalize(&path).unwrap_or_else(|_| path.clone()).to_string_lossy().to_lowercase();
                if !directories.insert(directory) { continue; }
                match fs::read_dir(&path) {
                    Ok(entries) => for entry in entries { match entry { Ok(e) => if recursive || !e.path().is_dir() { pending.push(e.path()); }, Err(e) => warnings.push(e.to_string()) } },
                    Err(e) => warnings.push(format!("{}：{e}", path.display())),
                }
            } else if meta.is_file() && path.extension().and_then(|v| v.to_str()).is_some_and(|v| v.eq_ignore_ascii_case("xlsx") || v.eq_ignore_ascii_case("xls")) && !path.file_name().unwrap_or_default().to_string_lossy().starts_with("~$") {
                let path = match fs::canonicalize(&path) { Ok(p) => p, Err(e) => { warnings.push(e.to_string()); continue; } };
                let key = path.to_string_lossy().to_lowercase();
                if seen.insert(key.clone()) { files.push(RenameFile { id: existing.get(&key).cloned().unwrap_or_else(id), name: path.file_name().unwrap_or_default().to_string_lossy().into(), path: path.to_string_lossy().into() }); }
                if files.len() > 10000 { return Err(AppError::new("LIMIT", "一次最多处理 10000 个文件，请分批添加")); }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        *self.files.lock().unwrap() = files.clone(); *self.book.lock().unwrap() = None; *self.plan.lock().unwrap() = None;
        Ok(RenameScan { files, warnings })
    }
    pub fn sheet_names(&self, file_id: &str) -> Result<Vec<String>> {
        let f = self.file(file_id)?; let book = Book::open(&f.path)?; Ok(book.workbook.sheet_names().to_vec())
    }
    fn cached<T>(&self, file: &RenameFile, work: impl FnOnce(&mut Book) -> Result<T>) -> Result<T> {
        if self.busy.load(Ordering::SeqCst) { return Err(AppError::new("BUSY", "批量任务处理中，稍后可继续预览")); }
        let mut cache = self.book.lock().unwrap();
        let mark = stamp(Path::new(&file.path))?;
        if cache.as_ref().is_none_or(|b| b.path != file.path || b.stamp != mark) { *cache = Some(Book::open(&file.path)?); }
        work(cache.as_mut().unwrap())
    }
    pub fn page(&self, file_id: &str, sheet: &str, offset: usize, limit: usize) -> Result<RenamePage> {
        self.page_window(file_id,sheet,offset,limit,0)
    }
    pub fn page_window(&self, file_id: &str, sheet: &str, offset: usize, limit: usize, column_offset: usize) -> Result<RenamePage> {
        let file = self.file(file_id)?;
        self.cached(&file, |book| {
            if column_offset>=16384 || offset>=1_048_576 {return Err(AppError::new("RANGE","预览位置超出Excel范围"));}
            let limit=limit.clamp(1,200);let window_width=(16384-column_offset).min(80);
            let scope=Scope {areas:vec![SheetRange {start_row:offset+1,end_row:(offset+limit).min(1_048_576),start_column:column_offset+1,end_column:column_offset+window_width}],filled_rows:vec![]};
            let data=crate::rename_read::read(&mut book.workbook,Path::new(&file.path),sheet,&scope,200*80)?;
            let height=data.rows.clamp(1,1_048_576);let declared_columns=data.columns.clamp(1,16384);
            let width=if declared_columns>column_offset {(declared_columns-column_offset).min(window_width)}else{window_width};
            let total_columns=declared_columns.max(column_offset+width);
            let end=if offset>=height {(offset+limit).min(1_048_576)}else{(offset+limit).min(height)};
            let rows = (offset..end).map(|row| (column_offset..column_offset+width).map(|col| checked_cell(&data,row+1,col+1).unwrap_or_else(|e| Cell::Text(format!("#{}",e.message)))).collect()).collect();
            let columns:Vec<Column>=(column_offset+1..=column_offset+width).map(|c|Column {key:format!("c{c}"),name:reader::cell_address(1,c).trim_end_matches('1').into(),data_type:ColumnType::Auto,suggested_type:ColumnType::Auto}).collect();
            let note=format!("当前预览 {}–{} 列，按行加载；核对只读取规则指定内容。",reader::cell_address(1,column_offset+1).trim_end_matches('1'),reader::cell_address(1,column_offset+width).trim_end_matches('1'));
            let mut appearance=crate::appearance::read_window(Path::new(&file.path),sheet,width,offset,limit,column_offset).unwrap_or_else(|e|SheetAppearance {widths:vec![110.;width],default_row_height:32.,show_grid_lines:true,note:Some(format!("原表格式无法显示：{}",e.message)),..Default::default()});
            appearance.note=Some(match appearance.note.take() {Some(old)=>format!("{note}；{old}"),None=>note.clone()});
            Ok(RenamePage {sheet:SourceSheet {id:format!("{}:{sheet}:{column_offset}",file.id),file_id:file.id.clone(),name:sheet.into(),rows:height,width,suggested_header_row:1,columns:columns.clone(),read_note:Some(note)},page:DataPage {columns,rows,row_numbers:(offset+1..=end).collect(),total:height,offset,sample:false},appearance,column_offset,total_columns})
        })
    }
    pub fn live(&self, file_id: &str, rules: &RenameRules) -> Result<RenameRow> {
        validate_rules(rules)?; let file = self.file(file_id)?;
        Ok(self.cached(&file, |book| row_for(&file, rules, &lookup_tables(rules), book,&RenameOutput::default())).unwrap_or_else(|e| failed_row(&file,e)))
    }
    pub fn prepare(&self, ids: Vec<String>, rules: RenameRules) -> Result<RenamePlan> {
        self.prepare_to(ids,rules,RenameOutput::default())
    }
    pub fn prepare_to(&self, ids:Vec<String>,rules:RenameRules,output:RenameOutput) -> Result<RenamePlan> {
        validate_rules(&rules)?;
        let output=normalize_output(output)?;
        let _job = self.begin("核对全部文件名", ids.len())?;
        *self.plan.lock().unwrap() = None; *self.book.lock().unwrap() = None;
        let mut rows = Vec::new(); let mut stamps = HashMap::new(); let mut seen = HashSet::new();
        let lookups = lookup_tables(&rules);
        for file_id in ids {
            if !seen.insert(file_id.clone()) { continue; }
            if self.cancelled.load(Ordering::SeqCst) { return Err(AppError::new("CANCELLED", "已停止核对，没有修改文件")); }
            let file = self.file(&file_id)?;
            self.progress.lock().unwrap().current = file.name.clone();
            let result = (|| { let mut book = Book::open(&file.path)?; let row = row_for(&file, &rules, &lookups, &mut book,&output)?;
                if stamp(Path::new(&file.path))? != book.stamp { return Err(AppError::new("CHANGED", "读取过程中源文件发生变化，请重新核对")); }
                stamps.insert(file_id, book.stamp.clone()); Ok(row) })();
            rows.push(result.unwrap_or_else(|e| failed_row(&file,e)));
            self.progress.lock().unwrap().done = rows.len();
        }
        if output.mode != RenameOutputMode::Original && output.number_conflicts { number_targets(&mut rows,&output)?; }
        let mut counts = HashMap::new();
        for r in &rows { if r.state == "ready" { *counts.entry(target_key(r)).or_insert(0) += 1; } }
        for r in &mut rows { if r.state == "ready" && counts.get(&target_key(r)).is_some_and(|v| *v > 1) { r.state = "skipped".into(); r.message = "本批有多个文件生成同一名称，请添加区分内容".into(); } }
        let value_labels = rules.parts.iter().map(|p| match p {
            RenamePart::Cell { source, .. } => format!("{}!{}",source.sheet,reader::cell_address(source.row,source.column)),
            RenamePart::Lookup { name, .. } => if name.is_empty() { "对照信息".into() } else { name.clone() },
            RenamePart::Text { .. } => "固定文字".into(), RenamePart::Condition { .. } => "条件标签".into(),
        }).collect();
        let plan = RenamePlan { id:id(), rows, journal_path:None, value_labels,output };
        *self.plan.lock().unwrap() = Some(SavedPlan { plan:plan.clone(), stamps,target_stamps:HashMap::new(),restored_stamps:HashMap::new() });
        Ok(plan)
    }
    pub fn execute(&self, plan_id: &str, root: &Path) -> Result<RenamePlan> {
        let _job = self.begin("执行重命名", 0)?;
        *self.book.lock().unwrap() = None;
        let mut slot = self.plan.lock().unwrap();
        if slot.as_ref().is_none_or(|p| p.plan.id != plan_id) { return Err(AppError::new("STALE", "预览已过期，请重新核对全部文件名")); }
        let mut saved = slot.take().unwrap(); drop(slot);
        if normalize_output(saved.plan.output.clone())?.directory!=saved.plan.output.directory {return Err(AppError::new("OUTPUT","目标文件夹位置已变化，请重新选择并核对"));}
        let dir = root.join("rename-history"); fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}_{}.jsonl", chrono::Local::now().format("%Y%m%d-%H%M%S-%3f"), saved.plan.id));
        saved.plan.journal_path = Some(path.to_string_lossy().into());
        let mut journal = OpenOptions::new().create_new(true).write(true).open(&path)?;
        write_record(&mut journal, &saved)?; // No source changes until the recovery record is durable.
        self.progress.lock().unwrap().total = saved.plan.rows.len();
        for row in &mut saved.plan.rows {
            self.progress.lock().unwrap().current = row.original_name.clone();
            if row.state == "ready" {
                if self.cancelled.load(Ordering::SeqCst) { row.state = "cancelled".into(); row.message = "已停止，文件未修改".into(); }
                else {
                    let source = Path::new(&row.path); let target = target_path(row);
                    let result = (|| {
                        let original=saved.stamps.get(&row.id).ok_or_else(||AppError::new("JOURNAL","缺少源文件校验记录，请重新核对"))?;
                        if original != &stamp(source)? { return Err(AppError::new("CHANGED", "源文件在预览后发生变化，请重新核对")); }
                        transfer_no_replace(source,&target,saved.plan.output.mode==RenameOutputMode::Copy,original,&self.cancelled,|mark| {
                            write_record(&mut journal,&JournalEvent {id:row.id.clone(),state:"publishing".into(),message:"准备写入目标文件".into(),target_stamp:Some(mark.clone()),restored_stamp:None})?;
                            saved.target_stamps.insert(row.id.clone(),mark.clone()); Ok(())
                        })
                    })();
                    match result {
                        Ok(TransferOutcome::Copied) => {row.state="copied".into();row.message="已集中保存，原文件保留".into();},
                        Ok(TransferOutcome::Moved) => {row.state=if saved.plan.output.mode==RenameOutputMode::Original {"renamed"} else {"moved"}.into();row.message="处理成功".into();},
                        Ok(TransferOutcome::CopyOnly(message)) => {row.state="copyOnly".into();row.message=message;},
                        Err(e) => { row.state = if e.code=="CANCELLED" {"cancelled"} else {"failed"}.into(); row.message = e.message; }
                    }
                    self.sync_file(row);
                }
                // If this fails, the durable intent plus file stamps allows recovery on restart.
                if let Err(e) = write_event(&mut journal, row, &row.state, &row.message) { return Err(AppError::new("JOURNAL", format!("日志写入失败，任务已停止。请打开上次记录核对已完成文件：{}；{}",path.display(),e.message))); }
            }
            self.progress.lock().unwrap().done += 1;
        }
        Ok(saved.plan)
    }
    pub fn latest(&self, root: &Path) -> Result<Option<RenamePlan>> { Ok(read_latest(root)?.map(|v| v.plan)) }
    pub fn undo(&self, run_id: &str, root: &Path) -> Result<RenamePlan> {
        let _job = self.begin("撤销上次处理", 0)?; *self.book.lock().unwrap() = None;
        let mut saved = read_latest(root)?.ok_or_else(|| AppError::new("HISTORY", "没有可恢复的重命名记录"))?;
        if saved.plan.id != run_id { return Err(AppError::new("STALE", "记录已变化，请重新打开上次记录")); }
        let mut journal = OpenOptions::new().append(true).open(saved.plan.journal_path.as_ref().unwrap())?;
        journal.write_all(b"\n")?; // Separate a possibly truncated final record before appending recovery events.
        self.progress.lock().unwrap().total = saved.plan.rows.len();
        for row in &mut saved.plan.rows {
            if matches!(row.state.as_str(), "renamed" | "moved" | "copied" | "copyOnly" | "undoFailed") && !self.cancelled.load(Ordering::SeqCst) {
                self.progress.lock().unwrap().current = row.new_name.clone();
                let target = target_path(row); let source=Path::new(&row.path);
                let result = (|| {
                    let original=saved.stamps.get(&row.id).ok_or_else(||AppError::new("JOURNAL","缺少源文件校验记录"))?;
                    let output=saved.target_stamps.get(&row.id).unwrap_or(original);
                    if output != &stamp(&target)? { return Err(AppError::new("CHANGED", "生成的文件已变化，已保留，未自动撤销")); }
                    // A completed copy is removable only while a verified original still exists.
                    let restored=saved.restored_stamps.get(&row.id);
                    let restored_ready=restored.is_some_and(|mark|stamp(source).ok().as_ref()==Some(mark));
                    let remove_copy=saved.plan.output.mode==RenameOutputMode::Copy || row.state=="copyOnly" || restored_ready || (saved.plan.output.mode==RenameOutputMode::Move && stamp(source).ok().as_ref()==Some(original));
                    if remove_copy {
                        if stamp(source).ok().as_ref()!=Some(if restored_ready {restored.unwrap()} else {original}) {return Err(AppError::new("CHANGED","原文件已变化或不存在，已保留目标文件，请人工核对"));}
                        write_event(&mut journal,row,"removingCopy","准备撤销生成的副本")?;
                        fs::remove_file(&target)?; return Ok(());
                    }
                    if source.try_exists()? {return Err(AppError::new("EXISTS","原路径已有文件，已保留两边文件，请另行处理"));}
                    if let Some(parent)=source.parent() {fs::create_dir_all(parent)?;}
                    let outcome=transfer_no_replace(&target,source,false,output,&self.cancelled,|mark| {
                        write_record(&mut journal,&JournalEvent {id:row.id.clone(),state:"restoring".into(),message:"准备恢复原路径".into(),target_stamp:None,restored_stamp:Some(mark.clone())})?;
                        saved.restored_stamps.insert(row.id.clone(),mark.clone()); Ok(())
                    })?;
                    if let TransferOutcome::CopyOnly(message)=outcome {return Err(AppError::new("RESTORE",format!("原路径已恢复，目标副本暂未移除：{message}")));}
                    Ok(())
                })();
                match result { Ok(()) => { row.state = "restored".into(); row.message = if saved.plan.output.mode==RenameOutputMode::Copy {"已撤销副本，原文件保留"} else {"已恢复原路径和原文件名"}.into(); }, Err(e) => { row.state = "undoFailed".into(); row.message = e.message; } }
                self.sync_file(row);
                write_event(&mut journal, row, &row.state, &row.message)?;
            }
            self.progress.lock().unwrap().done += 1;
        }
        Ok(saved.plan)
    }
    pub fn export(&self, plan: &RenamePlan, path: &Path) -> Result<String> {
        if plan.rows.len() > 10000 || plan.value_labels.len() > 64 || plan.rows.iter().any(|r| r.values.len() > 64) { return Err(AppError::new("LIMIT", "日志超过 10000 个文件或 64 项命名内容，请分批保存")); }
        if path.extension().and_then(|v| v.to_str()).is_none_or(|v| !v.eq_ignore_ascii_case("xlsx")) { return Err(AppError::new("EXPORT", "请保存为 .xlsx 文件")); }
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        let err = |e: rust_xlsxwriter::XlsxError| AppError::new("EXPORT", e.to_string());
        sheet.set_name("重命名日志与汇总").map_err(err)?;
        for (c, name) in ["原文件名", "新文件名", "状态", "说明", "来源路径", "取值明细", "校验结果", "目标路径"].iter().enumerate() { sheet.write_string(0,c as u16,*name).map_err(err)?; }
        for (i, row) in plan.rows.iter().enumerate() {
            for (c, value) in [&row.original_name,&row.new_name,state_label(&row.state),&row.message,&row.path,&row.values.join(" | "),&row.checks.join("；")].iter().enumerate() { sheet.write_string(i as u32+1,c as u16,*value).map_err(err)?; }
            sheet.write_string(i as u32+1,7,target_path(row).to_string_lossy()).map_err(err)?;
        }
        sheet.set_column_width(0,38).map_err(err)?; sheet.set_column_width(1,48).map_err(err)?; sheet.set_column_width(2,16).map_err(err)?; sheet.set_column_width(3,60).map_err(err)?;
        let summary = book.add_worksheet(); summary.set_name("提取信息").map_err(err)?;
        let headers = ["原文件名".to_string(), "新文件名".into(), "状态".into()].into_iter().chain(plan.value_labels.iter().cloned()).collect::<Vec<_>>();
        for (c, name) in headers.iter().enumerate() { summary.write_string(0, c as u16, name).map_err(err)?; summary.set_column_width(c as u16, if c < 2 { 40. } else { 22. }).map_err(err)?; }
        for (i, row) in plan.rows.iter().enumerate() {
            for (c, value) in [row.original_name.as_str(), row.new_name.as_str(), state_label(&row.state)].into_iter().chain(row.values.iter().map(String::as_str)).enumerate() { summary.write_string(i as u32 + 1, c as u16, value).map_err(err)?; }
        }
        let bytes = book.save_to_buffer().map_err(err)?;
        let mut file = OpenOptions::new().create_new(true).write(true).open(path).map_err(|e| AppError::new("EXPORT", format!("无法保存（同名文件不会覆盖，请另选名称）：{e}")))?;
        file.write_all(&bytes)?; file.sync_all()?; Ok(path.to_string_lossy().into())
    }
}
fn path_key(path:&Path)->String {path.to_string_lossy().to_lowercase()}
fn target_path(row:&RenameRow)->PathBuf {row.target_path.as_ref().map(PathBuf::from).unwrap_or_else(||Path::new(&row.path).with_file_name(&row.new_name))}
fn target_key(row: &RenameRow) -> String {path_key(&target_path(row))}
fn normalize_output(mut output:RenameOutput)->Result<RenameOutput> {
    if output.mode==RenameOutputMode::Original {output.directory=None;}
    else {
        let directory=output.directory.as_ref().filter(|v|!v.trim().is_empty()).ok_or_else(||AppError::new("OUTPUT","请先选择集中保存的文件夹"))?;
        let path=fs::canonicalize(directory).map_err(|e|AppError::new("OUTPUT",format!("目标文件夹无法访问，请重新选择：{e}")))?;
        if !path.is_dir() {return Err(AppError::new("OUTPUT","目标位置需要是一个文件夹"));}
        output.directory=Some(path.to_string_lossy().into());
    }
    Ok(output)
}
fn number_targets(rows:&mut [RenameRow],output:&RenameOutput)->Result<()> {
    let directory=Path::new(output.directory.as_ref().unwrap());
    let mut used=HashSet::new();
    for entry in fs::read_dir(directory)? {used.insert(entry?.file_name().to_string_lossy().to_lowercase());}
    let mut next=HashMap::<String,usize>::new();
    for row in rows.iter_mut().filter(|r|r.state=="ready") {
        let base=row.new_name.clone(); let key=base.to_lowercase();
        if used.contains(&key) {
            let name=Path::new(&base); let stem=name.file_stem().unwrap_or_default().to_string_lossy(); let extension=name.extension().unwrap_or_default().to_string_lossy();
            let counter=next.entry(key).or_insert(2);
            loop {row.new_name=format!("{stem} ({}).{extension}",*counter);*counter+=1;if !used.contains(&row.new_name.to_lowercase()) {break;}}
            row.message="目标重名，已自动编号".into();
        }
        let target=directory.join(&row.new_name);
        if row.new_name.encode_utf16().count()>240 || target.to_string_lossy().encode_utf16().count()>250 {
            row.state="skipped".into();row.message="集中后的路径过长，请缩短文件名或选择更短的目标路径".into();continue;
        }
        used.insert(row.new_name.to_lowercase());row.target_path=Some(target.to_string_lossy().into());
    }
    Ok(())
}
fn state_label(state: &str) -> &str { match state { "ready" => "待执行", "renamed" => "已重命名", "copied"=>"已复制", "moved"=>"已移动", "copyOnly"=>"副本已保存，原文件保留", "restored" => "已撤销", "unchanged" => "无需修改", "skipped" => "已跳过", "cancelled" => "未执行", "undoFailed" => "撤销失败", "failed" => "失败", _ => "需人工核对" } }
fn write_record(file: &mut File, value: &impl Serialize) -> Result<()> { serde_json::to_writer(&mut *file,value)?; file.write_all(b"\n")?; file.sync_data()?; Ok(()) }
fn write_event(file: &mut File, row: &RenameRow, state: &str, message: &str) -> Result<()> { write_record(file, &JournalEvent { id:row.id.clone(),state:state.into(),message:message.into(),target_stamp:None,restored_stamp:None }) }
fn move_no_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    // Deliberately omit MOVEFILE_REPLACE_EXISTING: an existence check alone races.
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" { fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32; }
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0) } != 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
    };
    #[cfg(not(windows))]
    let result = fs::hard_link(source,target).and_then(|_| fs::remove_file(source));
    result
}
enum TransferOutcome { Moved, Copied, CopyOnly(String) }
fn transfer_no_replace(source:&Path,target:&Path,copy:bool,original:&Stamp,cancelled:&AtomicBool,mut before_publish:impl FnMut(&Stamp)->Result<()>)->Result<TransferOutcome> {
    let check_cancel=|| if cancelled.load(Ordering::SeqCst) {Err(AppError::new("CANCELLED","已停止，源文件保留"))} else {Ok(())};
    check_cancel()?;
    if !copy {
        before_publish(original)?;
        match move_no_replace(source,target) {
            Ok(())=>return Ok(TransferOutcome::Moved),
            Err(e) if e.kind()==std::io::ErrorKind::CrossesDevices=>{},
            Err(e)=>return Err(AppError::new("RENAME",format!("未完成移动，请检查同名文件、占用或目录权限：{e}"))),
        }
    }
    let staged=target.with_file_name(format!(".gridflow-stage-{}.tmp",id()));
    let result=(|| {
        let mut input=File::open(source)?;
        let mut output=OpenOptions::new().write(true).create_new(true).open(&staged)?;
        let mut buffer=vec![0u8;1024*1024]; let mut written=0u64;
        loop {check_cancel()?;let size=input.read(&mut buffer)?;if size==0 {break;}output.write_all(&buffer[..size])?;written+=size as u64;}
        if &stamp(source)?!=original || written!=original.size {return Err(AppError::new("CHANGED","复制过程中源文件发生变化，未生成目标文件"));}
        output.set_times(FileTimes::new().set_modified(input.metadata()?.modified()?))?;
        output.sync_all()?;drop(output);drop(input);
        check_cancel()?;
        let mark=stamp(&staged)?;before_publish(&mark)?;
        move_no_replace(&staged,target).map_err(|e|AppError::new("COPY",format!("目标文件未写入，同名文件不会覆盖：{e}")))?;
        if copy {return Ok(TransferOutcome::Copied);}
        if cancelled.load(Ordering::SeqCst) || stamp(source).ok().as_ref()!=Some(original) {return Ok(TransferOutcome::CopyOnly("目标文件已保存；源文件发生变化或任务已停止，原文件保留".into()));}
        match fs::remove_file(source) {Ok(())=>Ok(TransferOutcome::Moved),Err(e)=>Ok(TransferOutcome::CopyOnly(format!("目标文件已保存，原文件未能移除，已保留两份：{e}")))}
    })();
    if staged.exists() {let _=fs::remove_file(&staged);}
    result
}
fn read_latest(root: &Path) -> Result<Option<SavedPlan>> {
    let dir = root.join("rename-history"); if !dir.exists() { return Ok(None); }
    let mut paths = fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|v| v.path())).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).collect::<Vec<_>>(); paths.sort();
    let Some(path) = paths.last() else { return Ok(None); };
    let mut lines = BufReader::new(File::open(path)?).split(b'\n');
    let first = lines.next().transpose()?.ok_or_else(|| AppError::new("JOURNAL", "重命名记录为空，请保留记录并检查磁盘"))?;
    let mut saved: SavedPlan = serde_json::from_slice(&first)?;
    for line in lines {
        let line = line?;
        if let Ok(event) = serde_json::from_slice::<JournalEvent>(&line) {
            if let Some(mark)=event.target_stamp {saved.target_stamps.insert(event.id.clone(),mark);}
            if let Some(mark)=event.restored_stamp {saved.restored_stamps.insert(event.id.clone(),mark);}
            if let Some(row) = saved.plan.rows.iter_mut().find(|r| r.id == event.id) { row.state = event.state; row.message = event.message; }
        }
    }
    saved.plan.journal_path = Some(path.to_string_lossy().into());
    for row in &mut saved.plan.rows {
        if matches!(row.state.as_str(), "renaming" | "publishing" | "restoring" | "removingCopy") {
            let source = Path::new(&row.path); let target = target_path(row);
            let original=saved.stamps.get(&row.id);
            let output=saved.target_stamps.get(&row.id).or(original);
            let restored=saved.restored_stamps.get(&row.id).or(original);
            let source_ok=original.is_some() && stamp(source).ok().as_ref()==original;
            let target_ok=output.is_some() && stamp(&target).ok().as_ref()==output;
            if matches!(row.state.as_str(),"restoring"|"removingCopy") && restored.is_some() && stamp(source).ok().as_ref()==restored {
                row.state=if !target.try_exists()? {"restored"} else if target_ok {"undoFailed"} else {"uncertain"}.into();row.message="撤销曾中断，请核对结果；保留的副本可再次撤销".into();
            } else if target_ok && (!source.try_exists()? || saved.plan.output.mode!=RenameOutputMode::Original) {
                row.state=match saved.plan.output.mode {RenameOutputMode::Copy=>"copied",RenameOutputMode::Move if source.try_exists()?=>"copyOnly",RenameOutputMode::Move=>"moved",RenameOutputMode::Original=>"renamed"}.into();row.message="根据操作记录核对目标文件；原文件如存在则保留".into();
            } else if !target.try_exists()? && source_ok {row.state="cancelled".into();row.message="原文件已保留".into();}
            else { row.state = "uncertain".into(); row.message = "任务曾中断且文件状态已变化，请人工核对原路径与新路径".into(); }
        } else if row.state == "ready" { row.state = "cancelled".into(); row.message = "任务中断，未执行".into(); }
    }
    Ok(Some(saved))
}
