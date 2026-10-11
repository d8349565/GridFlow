use crate::{model::*, reader};
use argon2::{
    password_hash::{PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use duckdb::{appender_params_from_iter, params, types::Value, Connection};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Instant,
};

pub struct Engine {
    pub root: PathBuf,
    connection: Mutex<Connection>,
    workspace: Mutex<Workspace>,
    pub(crate) transient: Mutex<HashMap<String, Dataset>>,
    cancelled: Mutex<HashMap<String, Arc<AtomicBool>>>,
    queue: mpsc::Sender<String>,
    pub(crate) heavy: Mutex<()>,
    unlocked: AtomicBool,
}
impl Engine {
    pub fn open(root: &Path) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root.join("imports"))?;
        std::fs::create_dir_all(root.join("results"))?;
        let connection = Connection::open(root.join("workspace.duckdb"))?;
        connection.execute_batch("SET memory_limit='256MB'; SET threads=2; SET autoinstall_known_extensions=false; SET autoload_known_extensions=false; CREATE TABLE IF NOT EXISTS meta (key VARCHAR PRIMARY KEY, value VARCHAR); CREATE TABLE IF NOT EXISTS issues (dataset_id VARCHAR, file_id VARCHAR, sheet_id VARCHAR, source_row BIGINT, column_name VARCHAR, original_value VARCHAR, message VARCHAR, source_column BIGINT, file_name VARCHAR, sheet_name VARCHAR);")?;
        connection.execute_batch("ALTER TABLE issues ADD COLUMN IF NOT EXISTS source_column BIGINT DEFAULT 1; ALTER TABLE issues ADD COLUMN IF NOT EXISTS file_name VARCHAR DEFAULT ''; ALTER TABLE issues ADD COLUMN IF NOT EXISTS sheet_name VARCHAR DEFAULT '';")?;
        let saved: Option<String> = connection
            .query_row("SELECT value FROM meta WHERE key='workspace'", [], |r| {
                r.get(0)
            })
            .ok();
        let mut workspace = if let Some(saved) = saved {
            serde_json::from_str::<Workspace>(&saved)?
        } else {
            Workspace::default()
        };
        // 兼容开发期间已有的 v2 数据表；保留原文本，并补充独立数值投影。
        for dataset in &workspace.datasets {
            for i in 0..dataset.columns.len() {
                connection.execute_batch(&format!("ALTER TABLE d_{} ADD COLUMN IF NOT EXISTS n{i} DOUBLE; UPDATE d_{} SET n{i}=TRY_CAST(c{i} AS DOUBLE) WHERE n{i} IS NULL AND t{i}=2",dataset.id,dataset.id))?;
            }
        }
        for task in &mut workspace.tasks {
            if matches!(
                task.state,
                TaskState::Pending | TaskState::Running | TaskState::Cancelling
            ) {
                task.state = TaskState::Interrupted;
                task.phase = "程序关闭，任务已中断；可重试".into();
                task.finished_at = Some(now());
            }
            if let Some(ref ds_id) = task.dataset_id {
                if !workspace.datasets.iter().any(|d| &d.id == ds_id) {
                    task.dataset_id = None;
                }
            }
        }
        workspace.data_directory = root.to_string_lossy().into();
        workspace.locked = workspace.settings.password_hash.is_some();
        for file in &mut workspace.files {
            if let Some(name) = Path::new(&file.stored_path).file_name() {
                file.stored_path = root.join("imports").join(name).to_string_lossy().into();
            }
        }
        let unlocked = !workspace.locked;
        let (queue, receiver) = mpsc::channel();
        let engine = Arc::new(Self {
            root: root.into(),
            connection: Mutex::new(connection),
            workspace: Mutex::new(workspace),
            transient: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashMap::new()),
            queue,
            heavy: Mutex::new(()),
            unlocked: AtomicBool::new(unlocked),
        });
        engine.persist()?;
        let weak = Arc::downgrade(&engine);
        std::thread::spawn(move || {
            while let Ok(task_id) = receiver.recv() {
                if let Some(engine) = weak.upgrade() {
                    engine.execute_task(&task_id);
                } else {
                    break;
                }
            }
        });
        Ok(engine)
    }
    pub(crate) fn conn(&self) -> Result<Connection> {
        Ok(self
            .connection
            .lock()
            .map_err(|_| AppError::new("INTERNAL", "数据库状态异常"))?
            .try_clone()?)
    }
    pub fn guard(&self) -> Result<()> {
        if self.unlocked.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AppError::new("LOCKED", "工作台已锁定，请输入密码解锁"))
        }
    }
    fn persist_locked(&self, ws: &Workspace) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR REPLACE INTO meta VALUES ('workspace', ?)",
            params![serde_json::to_string(ws)?],
        )?;
        Ok(())
    }
    fn persist(&self) -> Result<()> {
        self.persist_locked(&self.workspace.lock().unwrap())
    }
    pub fn snapshot(&self) -> Result<Workspace> {
        let ws = self.workspace.lock().unwrap();
        if !self.unlocked.load(Ordering::SeqCst) {
            return Ok(Workspace {
                locked: true,
                data_directory: ws.data_directory.clone(),
                ..Workspace::default()
            });
        }
        let mut copy = ws.clone();
        copy.settings.password_hash = ws.settings.password_hash.as_ref().map(|_| "enabled".into());
        copy.locked = false;
        Ok(copy)
    }
    pub fn save_draft(&self, rules: RuleSet, active_sheet_id: Option<String>) -> Result<()> {
        self.guard()?;
        let mut ws = self.workspace.lock().unwrap();
        let mut rules = rules;
        let valid = ws
            .files
            .iter()
            .flat_map(|f| &f.sheets)
            .map(|s| s.id.clone())
            .collect::<HashSet<_>>();
        rules.sources.retain(|s| valid.contains(&s.sheet_id));
        for file in &ws.files {
            for sheet in &file.sheets {
                if !rules.sources.iter().any(|s| s.sheet_id == sheet.id) {
                    rules.sources.push(SourceSelection {
                        sheet_id: sheet.id.clone(),
                        enabled: false,
                        header_row: 1,
                        header_range: None,
                        source_name: format!("{} / {}", file.name, sheet.name),
                    });
                }
            }
        }
        ws.rules = rules;
        ws.active_sheet_id = active_sheet_id.filter(|id| valid.contains(id));
        self.persist_locked(&ws)
    }
    pub fn unlock(&self, password: &str) -> Result<()> {
        let ws = self.workspace.lock().unwrap();
        if let Some(hash) = &ws.settings.password_hash {
            let hash =
                PasswordHash::new(hash).map_err(|_| AppError::new("PASSWORD", "密码设置损坏"))?;
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .map_err(|_| AppError::new("PASSWORD", "密码不正确"))?;
        }
        self.unlocked.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn set_password(&self, password: &str, current_password: &str) -> Result<()> {
        self.guard()?;
        self.unlock(current_password)?;
        if !password.is_empty() && password.chars().count() < 6 {
            return Err(AppError::new("PASSWORD", "密码至少 6 个字符"));
        }
        let hash = if password.is_empty() {
            None
        } else {
            let salt = SaltString::generate(&mut rand_core::OsRng);
            Some(
                Argon2::default()
                    .hash_password(password.as_bytes(), &salt)
                    .map_err(|e| AppError::new("PASSWORD", e.to_string()))?
                    .to_string(),
            )
        };
        let mut ws = self.workspace.lock().unwrap();
        ws.settings.password_hash = hash;
        self.persist_locked(&ws)
    }
    pub fn lock(&self) -> Result<()> {
        if self
            .workspace
            .lock()
            .unwrap()
            .settings
            .password_hash
            .is_some()
        {
            self.unlocked.store(false, Ordering::SeqCst);
        }
        Ok(())
    }
    pub fn import_files(&self, paths: Vec<String>) -> Result<Vec<ImportItem>> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let mut expanded = Vec::new();
        for path in paths {
            let p = PathBuf::from(&path);
            if p.is_dir() {
                for entry in std::fs::read_dir(p)? {
                    let p = entry?.path();
                    if p.is_file() && reader_extension(&p) {
                        expanded.push(p.to_string_lossy().into_owned());
                    }
                }
            } else {
                expanded.push(path);
            }
        }
        let total: u64 = expanded
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        if total > 500 * 1024 * 1024 {
            return Err(AppError::new("LIMIT", "本次导入超过 500MB，请分批导入"));
        }
        let mut items = Vec::new();
        for path in expanded {
            let result = self.import_one(Path::new(&path));
            items.push(match result {
                Ok(file) => ImportItem {
                    path,
                    file: Some(file),
                    error: None,
                },
                Err(e) => ImportItem {
                    path,
                    file: None,
                    error: Some(e.message),
                },
            });
        }
        Ok(items)
    }
    pub fn load_sample_files(&self) -> Result<Vec<ImportItem>> {
        self.guard()?;
        let samples_dir = self.root.join("samples");
        std::fs::create_dir_all(&samples_dir)?;
        let file1_path = samples_dir.join("示例_门店A_月度销售.csv");
        let file2_path = samples_dir.join("示例_门店B_月度销售.csv");
        let content1 = "\
门店名称：上海徐汇店,统计周期：2026年1月,填报人：张三
商品编号,商品名称,品类,销售数量,销售单价,销售额,退货数量
SPU-001,精品美式咖啡,咖啡饮品,1200,22,26400,12
SPU-002,生椰拿铁,咖啡饮品,2100,28,58800,18
SPU-101,法式原味可颂,西式烘焙,650,18,11700,5
SPU-102,提拉米苏切块,西式烘焙,420,32,13440,3
SPU-201,精品挂耳咖啡包,咖啡周边,180,68,12240,0
";
        let content2 = "\
门店名称：杭州西湖店,统计周期：2026年1月,填报人：李四
商品编号,商品名称,品类,销售数量,销售单价,销售额,退货数量
SPU-001,精品美式咖啡,咖啡饮品,1450,22,31900,15
SPU-002,生椰拿铁,咖啡饮品,2600,28,72800,20
SPU-101,法式原味可颂,西式烘焙,800,18,14400,8
SPU-102,提拉米苏切块,西式烘焙,510,32,16320,4
SPU-201,精品挂耳咖啡包,咖啡周边,220,68,14960,2
";
        std::fs::write(&file1_path, content1.as_bytes())?;
        std::fs::write(&file2_path, content2.as_bytes())?;
        self.import_files(vec![
            file1_path.to_string_lossy().into_owned(),
            file2_path.to_string_lossy().into_owned(),
        ])
    }
    fn import_one(&self, path: &Path) -> Result<SourceFile> {
        let size = std::fs::metadata(path)?.len();
        if size > 100 * 1024 * 1024 {
            return Err(AppError::new("LIMIT", "单文件超过 100MB，请拆分后导入"));
        }
        if !reader_extension(path) {
            return Err(AppError::new("UNSUPPORTED", "支持 .xlsx、.xls 和 .csv"));
        }
        let file_id = id();
        let stored = self.root.join("imports").join(format!(
            "{file_id}.{}",
            path.extension().unwrap().to_string_lossy()
        ));
        std::fs::copy(path, &stored)?;
        let result = (|| {
            let read = reader::read(&stored)?;
            let conn = self.conn()?;
            conn.execute_batch("BEGIN TRANSACTION")?;
            let parsed = (|| {
                let mut sheets = Vec::new();
                for sheet in read.sheets {
                    let sheet_id = id();
                    let table = format!("r_{sheet_id}");
                    create_table(&conn, &table, sheet.width, false)?;
                    let mut appender = conn.appender(&table)?;
                    for (row, cells) in sheet.rows.iter().enumerate() {
                        append_cells(&mut appender, row + 1, cells, sheet.width, None)?;
                    }
                    appender.flush()?;
                    drop(appender);
                    let header = reader::suggest_header(&sheet.rows);
                    let names = reader::header_names(
                        sheet.rows.get(header - 1).map(Vec::as_slice).unwrap_or(&[]),
                        sheet.width,
                    );
                    let columns = names
                        .into_iter()
                        .enumerate()
                        .map(|(i, key)| {
                            let values = sheet
                                .rows
                                .iter()
                                .skip(header)
                                .take(100)
                                .map(|r| r.get(i).cloned().unwrap_or(Cell::Empty))
                                .collect::<Vec<_>>();
                            let suggested_type = reader::suggest_column_type(&key, &values);
                            Column {
                                key: reader::business_key(&key),
                                name: key,
                                data_type: ColumnType::Auto,
                                suggested_type,
                            }
                        })
                        .collect();
                    sheets.push(SourceSheet {
                        id: sheet_id,
                        file_id: file_id.clone(),
                        name: sheet.name,
                        rows: sheet.rows.len(),
                        width: sheet.width,
                        suggested_header_row: header,
                        columns,
                    });
                }
                Ok::<_, AppError>(sheets)
            })();
            match parsed {
                Ok(sheets) => {
                    conn.execute_batch("COMMIT")?;
                    Ok(SourceFile {
                        id: file_id.clone(),
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        size,
                        imported_at: now(),
                        stored_path: stored.to_string_lossy().into(),
                        encoding: read.encoding,
                        sheets,
                    })
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(e)
                }
            }
        })();
        match result {
            Ok(file) => {
                let mut ws = self.workspace.lock().unwrap();
                for sheet in &file.sheets {
                    ws.rules.sources.push(SourceSelection {
                        sheet_id: sheet.id.clone(),
                        enabled: sheet.width > 0,
                        header_row: 1,
                        header_range: None,
                        source_name: format!("{} / {}", file.name, sheet.name),
                    });
                }
                if ws.active_sheet_id.is_none() {
                    ws.active_sheet_id = file.sheets.first().map(|s| s.id.clone());
                }
                activity(&mut ws, "导入文件", &file.name);
                ws.files.push(file.clone());
                self.persist_locked(&ws)?;
                Ok(file)
            }
            Err(e) => {
                let _ = std::fs::remove_file(stored);
                Err(e)
            }
        }
    }
    pub fn sheet(&self, sheet_id: &str) -> Result<SourceSheet> {
        self.guard()?;
        self.lookup_sheet(sheet_id)
    }
    fn lookup_sheet(&self, sheet_id: &str) -> Result<SourceSheet> {
        self.workspace
            .lock()
            .unwrap()
            .files
            .iter()
            .flat_map(|f| f.sheets.iter())
            .find(|s| s.id == sheet_id)
            .cloned()
            .ok_or_else(|| AppError::new("NOT_FOUND", "工作表不存在，请重新导入"))
    }
    pub fn preview_sheet(
        &self,
        sheet_id: &str,
        offset: usize,
        limit: usize,
        header_row: Option<usize>,
    ) -> Result<DataPage> {
        let sheet = self.sheet(sheet_id)?;
        let conn = self.conn()?;
        let table = format!("r_{}", sheet.id);
        let columns = if let Some(row) = header_row {
            self.columns_at(&sheet, row)?
        } else {
            (0..sheet.width)
                .map(|i| Column {
                    key: format!("c{i}"),
                    name: reader::cell_address(1, i + 1).trim_end_matches('1').into(),
                    data_type: ColumnType::Auto,
                    suggested_type: ColumnType::Auto,
                })
                .collect()
        };
        let start = offset + header_row.unwrap_or(0);
        let rows = read_rows(&conn, &table, sheet.width, start, limit.min(1000))?;
        Ok(DataPage {
            columns,
            rows: rows.iter().map(|(_, c)| c.clone()).collect(),
            row_numbers: rows.iter().map(|(r, _)| *r).collect(),
            total: sheet.rows.saturating_sub(header_row.unwrap_or(0)),
            offset,
            sample: false,
        })
    }
    pub fn preview_appearance(&self, sheet_id: &str, offset: usize, limit: usize) -> Result<SheetAppearance> {
        self.guard()?;
        self.appearance_internal(sheet_id,offset,limit)
    }
    fn appearance_internal(&self, sheet_id: &str, offset: usize, limit: usize) -> Result<SheetAppearance> {
        let sheet = self.lookup_sheet(sheet_id)?;
        let path = self.workspace.lock().unwrap().files.iter().find(|f| f.id == sheet.file_id)
            .map(|f| f.stored_path.clone()).ok_or_else(|| AppError::new("NOT_FOUND", "原文件副本不存在"))?;
        Ok(crate::appearance::read(Path::new(&path), &sheet.name, sheet.width, offset, limit).unwrap_or_else(|e| SheetAppearance {
            widths: vec![110.; sheet.width],default_row_height:32.,show_grid_lines:true, note: Some(format!("原表格式暂时无法显示，使用标准网格：{}",e.message)), ..Default::default()
        }))
    }
    fn padded_for_sheet(&self,sheet:&SourceSheet)->HashMap<(usize,usize),usize> {
        let path=self.workspace.lock().unwrap().files.iter().find(|f|f.id==sheet.file_id).map(|f|f.stored_path.clone());
        path.and_then(|p|crate::appearance::padded_cells(Path::new(&p),&sheet.name,sheet.width).ok()).unwrap_or_default()
    }
    pub fn columns_for_source(&self, sheet: &SourceSheet, source: &SourceSelection) -> Result<Vec<Column>> {
        let Some(area) = &source.header_range else { return self.columns_at(sheet, source.header_row); };
        if area.start_row == 0 || area.end_row != source.header_row || area.end_row < area.start_row || area.end_row > sheet.rows
            || area.end_row - area.start_row >= 32 || area.start_column == 0 || area.end_column < area.start_column || area.end_column > sheet.width {
            return Err(AppError::new("HEADER", format!("{} 的表头选区无效，请选择表格范围内最多 32 行",sheet.name)));
        }
        let conn = self.conn()?;
        let mut rows = read_rows(&conn, &format!("r_{}",sheet.id),sheet.width,area.start_row-1,area.end_row-area.start_row+101)?;
        let merges = self.appearance_internal(&sheet.id,area.start_row-1,area.end_row-area.start_row+1)?.merges;
        for merge in &merges {
            if merge.start_row < area.start_row && merge.end_row >= area.start_row && merge.start_column <= area.end_column && merge.end_column >= area.start_column
                && !rows.iter().any(|(r,_)| *r == merge.start_row) {
                rows.extend(read_rows(&conn,&format!("r_{}",sheet.id),sheet.width,merge.start_row-1,1)?);
            }
        }
        let mut labels = Vec::new();
        for col in area.start_column..=area.end_column {
            let mut parts = Vec::new();
            for row in area.start_row..=area.end_row {
                let anchor = merges.iter().find(|m| row >= m.start_row && row <= m.end_row && col >= m.start_column && col <= m.end_column);
                let (r,c) = anchor.map(|m| (m.start_row,m.start_column)).unwrap_or((row,col));
                let text = rows.iter().find(|(n,_)| *n == r).and_then(|(_,cells)| cells.get(c-1)).map(|v| v.text().trim().to_string()).unwrap_or_default();
                if !text.is_empty() && parts.last() != Some(&text) { parts.push(text); }
            }
            let label=parts.join(" / ");
            if label.chars().count()>1024 { return Err(AppError::new("HEADER",format!("{} 的第 {col} 列拼接名称过长，请缩小表头选区",sheet.name))); }
            labels.push(Cell::Text(label));
        }
        Ok(reader::header_names(&labels,labels.len()).into_iter().enumerate().map(|(index,name)| {
            let values = rows.iter().filter(|(r,_)| *r > area.end_row).filter_map(|(_,v)| v.get(area.start_column - 1 + index).cloned()).collect::<Vec<_>>();
            Column { key: reader::business_key(&name), suggested_type: reader::suggest_column_type(&name,&values), name, data_type: ColumnType::Auto }
        }).collect())
    }
    pub fn columns_at(&self, sheet: &SourceSheet, header_row: usize) -> Result<Vec<Column>> {
        if header_row == 0 || header_row > sheet.rows.max(1) {
            return Err(AppError::new(
                "HEADER",
                format!("{} 的表头行超出工作表范围", sheet.name),
            ));
        }
        if header_row == sheet.suggested_header_row {
            return Ok(sheet.columns.clone());
        }
        let conn = self.conn()?;
        let rows = read_rows(
            &conn,
            &format!("r_{}", sheet.id),
            sheet.width,
            header_row - 1,
            101,
        )?;
        let headers = reader::header_names(
            rows.first().map(|(_, v)| v.as_slice()).unwrap_or(&[]),
            sheet.width,
        );
        Ok(headers
            .into_iter()
            .enumerate()
            .map(|(i, key)| {
                let values = rows
                    .iter()
                    .skip(1)
                    .map(|(_, row)| row[i].clone())
                    .collect::<Vec<_>>();
                Column {
                    key: reader::business_key(&key),
                    name: key.clone(),
                    data_type: ColumnType::Auto,
                    suggested_type: reader::suggest_column_type(&key, &values),
                }
            })
            .collect())
    }
    pub fn available_columns(&self, rules: &RuleSet) -> Result<Vec<Column>> {
        self.guard()?;
        self.available_internal(rules)
    }
    fn available_internal(&self, rules: &RuleSet) -> Result<Vec<Column>> {
        let mut columns = Vec::new();
        let mut seen = HashSet::new();
        for source in rules.sources.iter().filter(|s| s.enabled) {
            let sheet = self.lookup_sheet(&source.sheet_id)?;
            for mut col in self.columns_for_source(&sheet, source)? {
                col.key = map_key(rules, &col.key);
                col.name = reader::field_label(&col.key);
                if seen.insert(col.key.clone()) {
                    columns.push(col);
                }
            }
        }
        for rule in &rules.fixed_cells {
            let key = format!("fixed:{}", rule.id);
            columns.push(Column {
                key,
                name: rule.name.clone(),
                data_type: ColumnType::Auto,
                suggested_type: ColumnType::Text,
            });
        }
        if rules.include_source {
            columns.push(Column {
                key: "__source".into(),
                name: if seen.contains(&reader::business_key("来自文件")) {
                    "来自文件（自动添加）"
                } else {
                    "来自文件"
                }
                .into(),
                data_type: ColumnType::Text,
                suggested_type: ColumnType::Text,
            });
        }
        Ok(columns)
    }
    pub fn input_columns(&self, rules: &RuleSet) -> Result<Vec<Column>> {
        self.guard()?;
        let mut cols = Vec::new();
        let mut seen = HashSet::new();
        for source in rules.sources.iter().filter(|s| s.enabled) {
            for col in self.columns_for_source(&self.lookup_sheet(&source.sheet_id)?, source)? {
                if seen.insert(col.key.clone()) {
                    cols.push(col);
                }
            }
        }
        Ok(cols)
    }
    pub fn suggestions(&self, rules: &RuleSet) -> Result<Vec<TypeSuggestion>> {
        self.guard()?;
        let mut output = Vec::new();
        let mut seen = HashSet::new();
        for source in rules.sources.iter().filter(|s| s.enabled) {
            let sheet = self.lookup_sheet(&source.sheet_id)?;
            let cols = self.columns_for_source(&sheet, source)?;
            let padded=if cols.iter().any(|c|c.suggested_type==ColumnType::Text){self.padded_for_sheet(&sheet)}else{HashMap::new()};
            let rows = read_rows(
                &self.conn()?,
                &format!("r_{}", sheet.id),
                sheet.width,
                source.header_row,
                100,
            )?;
            for (index, col) in cols.into_iter().enumerate() {
                let key = map_key(rules, &col.key);
                if !seen.insert(key.clone()) {
                    continue;
                }
                let label = reader::field_label(&key);
                if label.trim().ends_with(':') || label.trim().ends_with('：') {
                    continue;
                }
                let examples: Vec<_> = rows
                    .iter()
                    .map(|(row, r)| {
                        let col=index+source.header_range.as_ref().map(|v|v.start_column-1).unwrap_or(0);
                        (&r[col],padded.get(&(*row,col+1)).copied())
                    })
                    .filter(|(c,_)| !matches!(c, Cell::Empty))
                    .take(3)
                    .map(|(before,width)| {
                        let original=padded_identifier(before,width);
                        let converted = reader::convert_cell(
                            if col.suggested_type==ColumnType::Text{&original}else{before},
                            &col.suggested_type,
                            rules
                                .date_formats
                                .get(&key)
                                .map(String::as_str)
                                .unwrap_or("auto"),
                        );
                        ConversionExample {
                            before: before.clone(),
                            after: converted.clone().unwrap_or_else(|_| before.clone()),
                            error: converted.err(),
                        }
                    })
                    .collect();
                if examples.iter().any(|e| e.error.is_some()) {
                    continue;
                }
                output.push(TypeSuggestion {
                    key: key.clone(),
                    name: label,
                    suggested_type: col.suggested_type,
                    examples,
                });
            }
        }
        Ok(output)
    }
    pub fn save_scheme(&self, name: String, rules: RuleSet) -> Result<Scheme> {
        self.guard()?;
        let ws = self.workspace.lock().unwrap();
        let sheets = rules
            .sources
            .iter()
            .filter_map(|s| {
                ws.files
                    .iter()
                    .find(|f| f.sheets.iter().any(|sheet| sheet.id == s.sheet_id))
                    .and_then(|file| {
                        file.sheets
                            .iter()
                            .find(|sheet| sheet.id == s.sheet_id)
                            .map(|sheet| SheetPreset {
                                name: sheet.name.clone(),
                                header_row: s.header_row,
                                header_range: s.header_range.clone(),
                                enabled: s.enabled,
                                workbook_sheets: file
                                    .sheets
                                    .iter()
                                    .map(|s| s.name.clone())
                                    .collect(),
                            })
                    })
            })
            .collect();
        drop(ws);
        let mut stored = rules;
        stored.sources.clear();
        let mut scheme = Scheme {
            id: id(),
            name: if name.trim().is_empty() {
                format!("汇总方案 {}", chrono::Local::now().format("%m-%d %H:%M"))
            } else {
                name.trim().into()
            },
            rules: stored,
            sheets,
            updated_at: now(),
        };
        let mut ws = self.workspace.lock().unwrap();
        activity(&mut ws, "保存方案", &scheme.name);
        // 同名保存视为更新这套设置，而不是堆出一堆同名副本
        if let Some(index) = ws.schemes.iter().position(|s| s.name == scheme.name) {
            scheme.id = ws.schemes[index].id.clone();
            ws.schemes[index] = scheme.clone();
        } else {
            ws.schemes.push(scheme.clone());
        }
        self.persist_locked(&ws)?;
        Ok(scheme)
    }
    pub fn rename_scheme(&self, scheme_id: &str, name: String) -> Result<Scheme> {
        self.guard()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::new("NAME", "请填写设置名称"));
        }
        let mut ws = self.workspace.lock().unwrap();
        let scheme = ws
            .schemes
            .iter_mut()
            .find(|s| s.id == scheme_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "方案不存在"))?;
        scheme.name = name.into();
        scheme.updated_at = now();
        let copy = scheme.clone();
        activity(&mut ws, "重命名方案", &copy.name);
        self.persist_locked(&ws)?;
        Ok(copy)
    }
    pub fn apply_scheme(&self, scheme_id: &str, current: RuleSet) -> Result<RuleSet> {
        self.guard()?;
        let ws = self.workspace.lock().unwrap();
        let scheme = ws
            .schemes
            .iter()
            .find(|s| s.id == scheme_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "方案不存在"))?;
        let selected_files = current
            .sources
            .iter()
            .filter(|s| s.enabled)
            .filter_map(|source| {
                ws.files
                    .iter()
                    .find(|f| f.sheets.iter().any(|s| s.id == source.sheet_id))
                    .map(|f| f.id.clone())
            })
            .collect::<HashSet<_>>();
        let mut rules = scheme.rules.clone();
        rules.sources = current.sources;
        let mut missing = Vec::new();
        if !scheme.sheets.is_empty() {
            for source in &mut rules.sources {
                if let Some(file) = ws
                    .files
                    .iter()
                    .find(|f| f.sheets.iter().any(|s| s.id == source.sheet_id))
                {
                    let sheet = file
                        .sheets
                        .iter()
                        .find(|s| s.id == source.sheet_id)
                        .unwrap();
                    if let Some(preset) = scheme
                        .sheets
                        .iter()
                        .find(|p| p.name == sheet.name && preset_matches(file, p))
                    {
                        source.enabled = selected_files.contains(&sheet.file_id) && preset.enabled;
                        source.header_row = preset.header_row;
                        source.header_range = preset.header_range.clone();
                    } else {
                        source.enabled = false;
                    }
                }
            }
            for file in ws.files.iter().filter(|f| selected_files.contains(&f.id)) {
                let applicable = scheme
                    .sheets
                    .iter()
                    .filter(|p| preset_matches(file, p))
                    .collect::<Vec<_>>();
                if applicable.is_empty() {
                    missing.push(format!("{} 的工作表结构不匹配", file.name));
                }
                for preset in applicable.into_iter().filter(|p| p.enabled) {
                    if !file.sheets.iter().any(|s| s.name == preset.name) {
                        missing.push(format!("{} 缺少 {}", file.name, preset.name));
                    }
                }
            }
        }
        if !missing.is_empty() {
            return Err(AppError::new(
                "SCHEME_MISMATCH",
                format!("方案无法完整匹配：{}", missing.join("；")),
            ));
        }
        Ok(rules)
    }
    pub(crate) fn replace_scheme_sheets(
        &self,
        scheme_id: &str,
        sheets: Vec<SheetPreset>,
    ) -> Result<Scheme> {
        let mut ws = self.workspace.lock().unwrap();
        let scheme = ws
            .schemes
            .iter_mut()
            .find(|s| s.id == scheme_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "方案不存在"))?;
        scheme.sheets = sheets;
        let copy = scheme.clone();
        self.persist_locked(&ws)?;
        Ok(copy)
    }
    pub(crate) fn publish_imported(&self, mut dataset: Dataset, name: &str) -> Result<Dataset> {
        dataset.name = format!("历史结果 · {name}");
        let mut ws = self.workspace.lock().unwrap();
        ws.datasets.insert(0, dataset.clone());
        self.persist_locked(&ws)?;
        Ok(dataset)
    }
    pub fn submit(&self, rules: RuleSet) -> Result<Task> {
        self.guard()?;
        self.validate(&rules)?;
        if self.workspace.lock().unwrap().tasks.iter().any(|t| {
            matches!(
                t.state,
                TaskState::Pending | TaskState::Running | TaskState::Cancelling
            ) && t.rules == rules
        }) {
            return Err(AppError::new(
                "DUPLICATE_TASK",
                "相同规则的任务已在队列中，请等待完成或调整规则",
            ));
        }
        let task = Task {
            id: id(),
            state: TaskState::Pending,
            phase: "等待处理".into(),
            progress: 0,
            created_at: now(),
            finished_at: None,
            error: None,
            dataset_id: None,
            rules,
        };
        self.cancelled
            .lock()
            .unwrap()
            .insert(task.id.clone(), Arc::new(AtomicBool::new(false)));
        let mut ws = self.workspace.lock().unwrap();
        activity(&mut ws, "提交任务", &task.id);
        ws.tasks.insert(0, task.clone());
        self.persist_locked(&ws)?;
        drop(ws);
        self.queue
            .send(task.id.clone())
            .map_err(|_| AppError::new("TASK", "任务队列已关闭"))?;
        Ok(task)
    }
    pub fn retry(&self, task_id: &str) -> Result<Task> {
        self.guard()?;
        let rules = self
            .workspace
            .lock()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .map(|t| t.rules.clone())
            .ok_or_else(|| AppError::new("NOT_FOUND", "任务不存在"))?;
        self.submit(rules)
    }
    pub fn cancel(&self, task_id: &str) -> Result<()> {
        self.guard()?;
        if let Some(flag) = self.cancelled.lock().unwrap().get(task_id) {
            flag.store(true, Ordering::SeqCst);
        }
        let mut ws = self.workspace.lock().unwrap();
        let task = ws
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "任务不存在"))?;
        if matches!(task.state, TaskState::Pending | TaskState::Running) {
            task.state = TaskState::Cancelling;
            task.phase = "正在停止，等待安全边界".into();
        }
        self.persist_locked(&ws)
    }
    fn update_task(&self, task_id: &str, f: impl FnOnce(&mut Task)) {
        let mut ws = self.workspace.lock().unwrap();
        if let Some(task) = ws.tasks.iter_mut().find(|t| t.id == task_id) {
            f(task);
        }
        let _ = self.persist_locked(&ws);
    }
    fn execute_task(&self, task_id: &str) {
        let _gate = self.heavy.lock().unwrap();
        let task = self
            .workspace
            .lock()
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .cloned();
        let Some(task) = task else {
            return;
        };
        let flag = self
            .cancelled
            .lock()
            .unwrap()
            .get(task_id)
            .cloned()
            .unwrap_or_default();
        self.update_task(task_id, |t| {
            t.state = TaskState::Running;
            t.phase = "准备处理".into();
        });
        let result = self.process(&task.rules, false, &flag, |percent, message| {
            self.update_task(task_id, |t| {
                t.progress = percent;
                t.phase = message.into();
            })
        });
        match result {
            Ok(dataset) => {
                let mut ws = self.workspace.lock().unwrap();
                ws.datasets.insert(0, dataset.clone());
                activity(
                    &mut ws,
                    "汇总完成",
                    &format!("{} · {} 行", dataset.name, dataset.summary.output_rows),
                );
                if let Some(task) = ws.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.state = TaskState::Succeeded;
                    task.progress = 100;
                    task.phase = "处理完成，可以预览或导出".into();
                    task.dataset_id = Some(dataset.id);
                    task.finished_at = Some(now());
                }
                let _ = self.persist_locked(&ws);
            }
            Err(error) => self.update_task(task_id, |t| {
                t.state = if error.code == "CANCELLED" {
                    TaskState::Cancelled
                } else {
                    TaskState::Failed
                };
                t.error = Some(error.message.clone());
                t.phase = error.message;
                t.finished_at = Some(now());
            }),
        }
        self.cancelled.lock().unwrap().remove(task_id);
    }
    pub fn validate(&self, rules: &RuleSet) -> Result<()> {
        self.guard()?;
        self.validate_internal(rules)
    }
    fn validate_internal(&self, rules: &RuleSet) -> Result<()> {
        if rules.version != 2 {
            return Err(AppError::new("VERSION", "请先导入并转换旧版配置"));
        }
        if !rules.sources.iter().any(|s| s.enabled) {
            return Err(AppError::new("SOURCES", "请勾选至少一个工作表"));
        }
        if !["outer", "inner"].contains(&rules.merge_strategy.as_str()) {
            return Err(AppError::new("RULE", "不支持的合并方式"));
        }
        for mapping in &rules.mappings {
            if mapping.target_key == "__source" || mapping.target_key.starts_with("fixed:") {
                return Err(AppError::new(
                    "MAPPING",
                    "来源和固定值列不能作为业务列映射目标",
                ));
            }
        }
        let mut names = HashSet::new();
        for col in rules
            .columns
            .iter()
            .filter(|c| !c.hidden && c.data_type != ColumnType::Skip)
        {
            if !names.insert(col.name.trim()) {
                return Err(AppError::new(
                    "COLUMN",
                    format!("导出列名重复：{}", col.name),
                ));
            }
        }
        for rule in &rules.fixed_cells {
            reader::parse_address(&rule.address)?;
            if rule.name.trim().is_empty() {
                return Err(AppError::new("CELL", "固定单元格列需要名称"));
            }
        }
        for source in rules.sources.iter().filter(|s| s.enabled) {
            let sheet = self.lookup_sheet(&source.sheet_id)?;
            let cols = self.columns_for_source(&sheet, source)?;
            for key in &rules.empty_columns {
                if !cols
                    .iter()
                    .any(|c| reader::same_key(&map_key(rules, &c.key), key))
                {
                    return Err(AppError::new(
                        "FIELD",
                        format!("{} 缺少空行判断字段 {key}", sheet.name),
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn preview_rules(&self, rules: RuleSet) -> Result<Dataset> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let dataset = self.process(&rules, true, &AtomicBool::new(false), |_, _| {})?;
        let mut transient = self.transient.lock().unwrap();
        let conn = self.conn()?;
        for (id, _) in transient.drain() {
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS d_{id}"));
            let _ = conn.execute("DELETE FROM issues WHERE dataset_id=?", params![id]);
        }
        transient.insert(dataset.id.clone(), dataset.clone());
        Ok(dataset)
    }
    pub(crate) fn dataset(&self, dataset_id: &str) -> Result<Dataset> {
        self.guard()?;
        if let Some(ds) = self.transient.lock().unwrap().get(dataset_id).cloned() {
            return Ok(ds);
        }
        self.workspace
            .lock()
            .unwrap()
            .datasets
            .iter()
            .find(|d| d.id == dataset_id)
            .cloned()
            .ok_or_else(|| AppError::new("NOT_FOUND", "处理结果不存在或已删除"))
    }
    pub fn process(
        &self,
        rules: &RuleSet,
        sample: bool,
        cancel: &AtomicBool,
        progress: impl Fn(u8, &str),
    ) -> Result<Dataset> {
        self.validate_internal(rules)?;
        let started = Instant::now();
        let dataset_id = id();
        let table = format!("d_{dataset_id}");
        let conn = self.conn()?;
        let mut columns = self.available_internal(rules)?;
        if rules.merge_strategy == "inner" {
            let mut common: Option<HashSet<String>> = None;
            for source in rules.sources.iter().filter(|s| s.enabled) {
                let sheet = self.lookup_sheet(&source.sheet_id)?;
                let keys = self
                    .columns_for_source(&sheet, source)?
                    .into_iter()
                    .map(|c| map_key(rules, &c.key))
                    .collect::<HashSet<_>>();
                common = Some(match common {
                    Some(prev) => prev.intersection(&keys).cloned().collect(),
                    None => keys,
                });
            }
            columns.retain(|c| {
                c.key.starts_with("fixed:")
                    || c.key == "__source"
                    || common.as_ref().is_some_and(|keys| keys.contains(&c.key))
            });
        }
        let original = columns.clone();
        let mut ordered = Vec::new();
        for rule in &rules.columns {
            if let Some(mut column) = columns
                .iter()
                .find(|c| reader::same_key(&c.key, &rule.key))
                .cloned()
            {
                column.name = rule.name.clone();
                column.data_type = rule.data_type.clone();
                if column.data_type != ColumnType::Skip {
                    ordered.push(column);
                }
            }
        }
        for col in &columns {
            if !rules
                .columns
                .iter()
                .any(|r| reader::same_key(&r.key, &col.key))
            {
                ordered.push(col.clone());
            }
        }
        columns = ordered;
        if columns.is_empty() {
            return Err(AppError::new("COLUMN", "没有可处理的列"));
        }
        let mut output_names = HashSet::new();
        for column in &columns {
            if !rules
                .columns
                .iter()
                .any(|r| reader::same_key(&r.key, &column.key) && r.hidden)
                && !output_names.insert(column.name.trim())
            {
                return Err(AppError::new(
                    "COLUMN",
                    format!("输出列名重复：{}，请在列配置中调整名称", column.name),
                ));
            }
        }
        let sources = rules
            .sources
            .iter()
            .filter(|s| s.enabled)
            .collect::<Vec<_>>();
        let files = self.workspace.lock().unwrap().files.clone();
        let result = (|| {
            conn.execute_batch("BEGIN TRANSACTION")?;
            create_table(&conn, &table, columns.len(), true)?;
            let mut output = conn.appender(&table)?;
            let mut issue_writer = conn.appender("issues")?;
            let mut summary = Summary::default();
            let mut sequence = 0usize;
            for (source_index, source) in sources.iter().enumerate() {
                check_cancel(cancel, started)?;
                let sheet = self.lookup_sheet(&source.sheet_id)?;
                let raw_columns = self.columns_for_source(&sheet, source)?;
                let padded=if raw_columns.iter().any(|c|c.suggested_type==ColumnType::Text){self.padded_for_sheet(&sheet)}else{HashMap::new()};
                let names = raw_columns
                    .iter()
                    .map(|c| map_key(rules, &c.key))
                    .collect::<Vec<_>>();
                if names.iter().collect::<HashSet<_>>().len() != names.len() {
                    return Err(AppError::new(
                        "MAPPING",
                        format!("{} 中多个输入列映射到同一列，请调整列映射", sheet.name),
                    ));
                }
                let mut fixed = HashMap::new();
                for rule in &rules.fixed_cells {
                    let key = format!("fixed:{}", rule.id);
                    let fixed_sheet = files
                        .iter()
                        .find(|f| f.id == sheet.file_id)
                        .and_then(|f| f.sheets.iter().find(|s| s.name == rule.sheet_name));
                    let value = if let Some(fixed_sheet) = fixed_sheet {
                        let (r, c) = reader::parse_address(&rule.address)?;
                        if c > fixed_sheet.width {
                            Cell::Empty
                        } else {
                            read_rows(
                                &conn,
                                &format!("r_{}", fixed_sheet.id),
                                fixed_sheet.width,
                                r - 1,
                                1,
                            )?
                            .first()
                            .and_then(|(_, v)| v.get(c - 1))
                            .cloned()
                            .unwrap_or(Cell::Empty)
                        }
                    } else {
                        return Err(AppError::new(
                            "MISSING_SHEET",
                            format!(
                                "文件 {} 缺少固定值工作表 {}",
                                files
                                    .iter()
                                    .find(|f| f.id == sheet.file_id)
                                    .map(|f| f.name.as_str())
                                    .unwrap_or(""),
                                rule.sheet_name
                            ),
                        ));
                    };
                    let (fixed_row, fixed_col) = reader::parse_address(&rule.address)?;
                    fixed.insert(
                        key,
                        (
                            value,
                            fixed_sheet.unwrap().id.clone(),
                            fixed_row,
                            fixed_col,
                            rule.sheet_name.clone(),
                        ),
                    );
                }
                let sql = format!(
                    "SELECT * FROM r_{} WHERE _row>? ORDER BY _row {}",
                    sheet.id,
                    if sample { "LIMIT 200" } else { "" }
                );
                let mut statement = conn.prepare(&sql)?;
                let mut cursor = statement.query(params![source.header_row as i64])?;
                while let Some(row) = cursor.next()? {
                    if sequence % 256 == 0 {
                        check_cancel(cancel, started)?;
                    }
                    let source_row = row.get::<_, i64>(0)? as usize;
                    let raw = (0..sheet.width)
                        .map(|i| cell_from_row(row, 1 + 2 * i))
                        .collect::<Result<Vec<_>>>()?;
                    let raw = if let Some(area) = &source.header_range { raw[area.start_column-1..area.end_column].to_vec() } else { raw };
                    summary.input_rows += 1;
                    let empty = if rules.empty_columns.is_empty() {
                        raw.iter().all(|c| c.text().trim().is_empty())
                    } else {
                        rules.empty_columns.iter().all(|key| {
                            names
                                .iter()
                                .position(|k| reader::same_key(k, key))
                                .and_then(|i| raw.get(i))
                                .is_none_or(|c| c.text().trim().is_empty())
                        })
                    };
                    if rules.remove_empty_rows && empty {
                        summary.empty_removed += 1;
                        continue;
                    }
                    let mut cells = Vec::new();
                    let mut numbers = Vec::new();
                    for column in &columns {
                        let mut raw_cell = if column.key == "__source" {
                            Cell::Text(source.source_name.clone())
                        } else if let Some(value) = fixed.get(&column.key) {
                            value.0.clone()
                        } else {
                            names
                                .iter()
                                .position(|k| k == &column.key)
                                .and_then(|i| raw.get(i))
                                .cloned()
                                .unwrap_or(Cell::Empty)
                        };
                        if [ColumnType::Auto,ColumnType::Text].contains(&column.data_type) {
                            if let Some(i)=names.iter().position(|k|k==&column.key) {
                                if raw_columns[i].suggested_type==ColumnType::Text {
                                    let c=i+source.header_range.as_ref().map(|r|r.start_column).unwrap_or(1);
                                    raw_cell=padded_identifier(&raw_cell,padded.get(&(source_row,c)).copied());
                                }
                            }
                        }
                        let mut cell = raw_cell.clone();
                        let mut error = None;
                        if matches!(&raw_cell,Cell::Text(s) if s=="#公式无缓存"||s.starts_with("#Excel错误:"))
                        {
                            error = Some(
                                "源单元格没有有效缓存值，请在 Excel 中重新计算并保存".to_string(),
                            );
                        }
                        if !matches!(raw_cell, Cell::Empty) {
                            let auto_hint = original
                                .iter()
                                .find(|c| c.key == column.key)
                                .map(|c| &c.suggested_type);
                            let ty = if column.data_type == ColumnType::Auto
                                && rules.clean_numeric
                                && auto_hint == Some(&ColumnType::Number)
                            {
                                ColumnType::Number
                            } else if column.data_type == ColumnType::Auto
                                && rules.parse_dates
                                && auto_hint == Some(&ColumnType::Date)
                            {
                                ColumnType::Date
                            } else {
                                column.data_type.clone()
                            };
                            match reader::convert_cell(
                                &raw_cell,
                                &ty,
                                rules
                                    .date_formats
                                    .iter()
                                    .find(|(key, _)| reader::same_key(key, &column.key))
                                    .map(|(_, value)| value.as_str())
                                    .unwrap_or("auto"),
                            ) {
                                Ok(converted) => cell = converted,
                                Err(message) => error = Some(message),
                            }
                        }
                        if let Some(message) = error {
                            summary.issue_count += 1;
                            let (issue_sheet, issue_row, issue_col, issue_sheet_name) =
                                if let Some(value) = fixed.get(&column.key) {
                                    (value.1.clone(), value.2, value.3, value.4.clone())
                                } else {
                                    (
                                        sheet.id.clone(),
                                        source_row,
                                        names
                                            .iter()
                                            .position(|k| k == &column.key)
                                            .map(|i| i + source.header_range.as_ref().map(|r| r.start_column).unwrap_or(1))
                                            .unwrap_or(1),
                                        sheet.name.clone(),
                                    )
                                };
                            let file_name = files
                                .iter()
                                .find(|f| f.id == sheet.file_id)
                                .map(|f| f.name.clone())
                                .unwrap_or_default();
                            issue_writer.append_row(params![
                                dataset_id,
                                sheet.file_id,
                                issue_sheet,
                                issue_row as i64,
                                column.name,
                                raw_cell.text(),
                                message,
                                issue_col as i64,
                                file_name,
                                issue_sheet_name
                            ])?;
                        }
                        let hint = original
                            .iter()
                            .find(|c| c.key == column.key)
                            .map(|c| &c.suggested_type);
                        let numeric = column.data_type == ColumnType::Number
                            || (column.data_type == ColumnType::Auto
                                && hint == Some(&ColumnType::Number));
                        numbers.push(if numeric {
                            reader::parse_number(&cell.text())
                        } else {
                            None
                        });
                        cells.push(cell);
                    }
                    sequence += 1;
                    append_cells(
                        &mut output,
                        sequence,
                        &cells,
                        columns.len(),
                        Some((&sheet.file_id, &sheet.id, source_row, &numbers)),
                    )?;
                }
                progress(
                    (10 + 75 * (source_index + 1) / sources.len()) as u8,
                    &format!("已处理 {} / {} 个工作表", source_index + 1, sources.len()),
                );
            }
            output.flush()?;
            drop(output);
            issue_writer.flush()?;
            drop(issue_writer);
            check_cancel(cancel, started)?;
            if rules.remove_duplicates {
                let selected = if rules.duplicate_columns.is_empty() {
                    columns
                        .iter()
                        .filter(|c| c.key != "__source")
                        .map(|c| c.key.clone())
                        .collect::<Vec<_>>()
                } else {
                    rules.duplicate_columns.clone()
                };
                let mut expressions = Vec::new();
                for key in selected {
                    let index = columns
                        .iter()
                        .position(|c| reader::same_key(&c.key, &key))
                        .ok_or_else(|| AppError::new("DEDUPE", format!("去重字段不存在：{key}")))?;
                    expressions.extend([format!("c{index}"), format!("t{index}")]);
                }
                if expressions.is_empty() {
                    return Err(AppError::new("DEDUPE", "请勾选至少一个业务去重字段"));
                }
                let keep = if rules.keep_strategy == "last" {
                    "DESC"
                } else {
                    "ASC"
                };
                conn.execute_batch(&format!("DELETE FROM {table} WHERE _row IN (SELECT _row FROM (SELECT _row, ROW_NUMBER() OVER (PARTITION BY {} ORDER BY _row {keep}) AS _rank FROM {table}) WHERE _rank>1)",expressions.join(",")))?;
            }
            summary.output_rows =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })? as usize;
            summary.duplicates_removed = sequence - summary.output_rows;
            summary.elapsed_ms = started.elapsed().as_millis() as u64;
            check_cancel(cancel, started)?;
            conn.execute_batch("COMMIT")?;
            Ok(Dataset {
                id: dataset_id.clone(),
                name: format!(
                    "{} {}",
                    if sample {
                        "规则预览"
                    } else {
                        "汇总结果"
                    },
                    chrono::Local::now().format("%m-%d %H:%M")
                ),
                created_at: now(),
                columns,
                summary,
                sample,
                rules: rules.clone(),
                exports: vec![],
            })
        })();
        if result.is_err() {
            let _ = conn.execute_batch("ROLLBACK");
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {table}"));
            let _ = conn.execute("DELETE FROM issues WHERE dataset_id=?", params![dataset_id]);
        }
        result
    }
    pub fn issues(&self, dataset_id: &str, offset: usize) -> Result<Vec<Issue>> {
        self.dataset(dataset_id)?;
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM issues WHERE dataset_id=? LIMIT 200 OFFSET ?")?;
        let rows = stmt.query_map(params![dataset_id, offset as i64], |r| {
            Ok(Issue {
                dataset_id: r.get(0)?,
                file_id: r.get(1)?,
                sheet_id: r.get(2)?,
                row: r.get::<_, i64>(3)? as usize,
                column: r.get(4)?,
                original_value: r.get(5)?,
                message: r.get(6)?,
                source_column: r.get::<_, i64>(7)? as usize,
                file_name: r.get(8)?,
                sheet_name: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn delete_items(&self, kind: &str, ids: &[String]) -> Result<()> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let mut current = self.workspace.lock().unwrap();
        let mut ws = current.clone();
        let conn = self.conn()?;
        let mut paths = Vec::new();
        if kind == "files" {
            let sheets = ws
                .files
                .iter()
                .filter(|f| ids.contains(&f.id))
                .flat_map(|f| &f.sheets)
                .map(|s| s.id.clone())
                .collect::<HashSet<_>>();
            if ws.tasks.iter().any(|t| {
                matches!(
                    t.state,
                    TaskState::Pending | TaskState::Running | TaskState::Cancelling
                ) && t
                    .rules
                    .sources
                    .iter()
                    .any(|s| s.enabled && sheets.contains(&s.sheet_id))
            }) {
                return Err(AppError::new(
                    "TASK_CONFLICT",
                    "任务仍在使用选中文件，请先完成或取消任务",
                ));
            }
        }
        conn.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| {
            if kind == "datasets" {
                for ds in ws.datasets.iter().filter(|d| ids.contains(&d.id)) {
                    conn.execute_batch(&format!("DROP TABLE IF EXISTS d_{}", ds.id))?;
                    conn.execute("DELETE FROM issues WHERE dataset_id=?", params![ds.id])?;
                    paths.extend(ds.exports.iter().map(PathBuf::from));
                }
                ws.datasets.retain(|d| !ids.contains(&d.id));
                for task in &mut ws.tasks {
                    if task.dataset_id.as_ref().is_some_and(|id| ids.contains(id)) {
                        task.dataset_id = None;
                    }
                }
            } else if kind == "files" {
                for file in ws.files.iter().filter(|f| ids.contains(&f.id)) {
                    for sheet in &file.sheets {
                        conn.execute_batch(&format!("DROP TABLE IF EXISTS r_{}", sheet.id))?;
                    }
                    paths.push(PathBuf::from(&file.stored_path));
                }
                ws.files.retain(|f| !ids.contains(&f.id));
                let valid = ws
                    .files
                    .iter()
                    .flat_map(|f| &f.sheets)
                    .map(|s| s.id.clone())
                    .collect::<HashSet<_>>();
                ws.rules.sources.retain(|s| valid.contains(&s.sheet_id));
                if ws
                    .active_sheet_id
                    .as_ref()
                    .is_some_and(|s| !valid.contains(s))
                {
                    ws.active_sheet_id = valid.into_iter().next();
                }
            } else if kind == "schemes" {
                ws.schemes.retain(|s| !ids.contains(&s.id));
            } else if kind == "activities" {
                ws.activities.retain(|a| !ids.contains(&a.id));
            } else {
                return Err(AppError::new("DELETE", "不支持的清理类别"));
            }
            activity(&mut ws, "手动清理", &format!("{kind} · {} 项", ids.len()));
            conn.execute(
                "INSERT OR REPLACE INTO meta VALUES ('workspace', ?)",
                params![serde_json::to_string(&ws)?],
            )?;
            conn.execute_batch("COMMIT")?;
            Ok::<_, AppError>(())
        })();
        if let Err(error) = result {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error);
        }
        *current = ws;
        drop(current);
        let imports = self.root.join("imports").canonicalize()?;
        let results = self.root.join("results").canonicalize()?;
        let mut failures = Vec::new();
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let resolved = path.canonicalize()?;
            if resolved.starts_with(&imports) || resolved.starts_with(&results) {
                if let Err(e) = std::fs::remove_file(&resolved) {
                    failures.push(format!("{}：{e}", resolved.display()));
                }
            }
        }
        if !failures.is_empty() {
            return Err(AppError::new(
                "CLEANUP_PARTIAL",
                format!(
                    "记录已清理，但副本删除失败，请在数据目录手动处理：{}",
                    failures.join("；")
                ),
            ));
        }
        Ok(())
    }
    pub fn disk_usage(&self) -> Result<u64> {
        self.guard()?;
        fn size(p: &Path) -> std::io::Result<u64> {
            let mut total = 0;
            for e in std::fs::read_dir(p)? {
                let p = e?.path();
                total += if p.is_dir() {
                    size(&p)?
                } else {
                    std::fs::metadata(p)?.len()
                };
            }
            Ok(total)
        }
        Ok(size(&self.root)?)
    }
    pub(crate) fn record_export(&self, dataset_id: &str, path: &Path) -> Result<()> {
        let mut ws = self.workspace.lock().unwrap();
        if let Some(ds) = ws.datasets.iter_mut().find(|d| d.id == dataset_id) {
            ds.exports.push(path.to_string_lossy().into());
        }
        ws.settings.last_export_directory = path.parent().map(|p| p.to_string_lossy().into());
        activity(&mut ws, "导出结果", &path.to_string_lossy());
        self.persist_locked(&ws)
    }
}
pub(crate) fn create_table(
    conn: &Connection,
    table: &str,
    width: usize,
    origin: bool,
) -> Result<()> {
    let mut definitions = vec!["_row BIGINT".into()];
    for i in 0..width {
        definitions.push(format!("c{i} VARCHAR"));
        definitions.push(format!("t{i} UTINYINT"));
        if origin {
            definitions.push(format!("n{i} DOUBLE"));
        }
    }
    if origin {
        definitions.extend([
            "_file VARCHAR".into(),
            "_sheet VARCHAR".into(),
            "_source_row BIGINT".into(),
        ]);
    }
    conn.execute_batch(&format!("CREATE TABLE {table} ({})", definitions.join(",")))?;
    Ok(())
}
pub(crate) fn append_cells(
    appender: &mut duckdb::Appender<'_>,
    row: usize,
    cells: &[Cell],
    width: usize,
    origin: Option<(&str, &str, usize, &[Option<f64>])>,
) -> Result<()> {
    let mut values = vec![Value::BigInt(row as i64)];
    for i in 0..width {
        let cell = cells.get(i).unwrap_or(&Cell::Empty);
        values.push(if matches!(cell, Cell::Empty) {
            Value::Null
        } else {
            Value::Text(cell.text())
        });
        values.push(Value::UTinyInt(cell.kind()));
        if let Some((_, _, _, numbers)) = origin {
            values.push(
                numbers
                    .get(i)
                    .copied()
                    .flatten()
                    .map(Value::Double)
                    .unwrap_or(Value::Null),
            );
        }
    }
    if let Some((file, sheet, row, _)) = origin {
        values.extend([
            Value::Text(file.into()),
            Value::Text(sheet.into()),
            Value::BigInt(row as i64),
        ]);
    }
    appender.append_row(appender_params_from_iter(values))?;
    Ok(())
}
pub(crate) fn cell_from_row(row: &duckdb::Row<'_>, index: usize) -> Result<Cell> {
    Ok(Cell::from_parts(row.get(index)?, row.get(index + 1)?))
}
pub(crate) fn read_rows(
    conn: &Connection,
    table: &str,
    width: usize,
    offset: usize,
    limit: usize,
) -> Result<Vec<(usize, Vec<Cell>)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT * FROM {table} WHERE _row>? ORDER BY _row LIMIT ?"
    ))?;
    let mut cursor = stmt.query(params![offset as i64, limit as i64])?;
    let mut output = Vec::new();
    while let Some(row) = cursor.next()? {
        output.push((
            row.get::<_, i64>(0)? as usize,
            (0..width)
                .map(|i| cell_from_row(row, 1 + 2 * i))
                .collect::<Result<_>>()?,
        ));
    }
    Ok(output)
}
fn reader_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| ["xlsx", "xls", "csv"].contains(&s.to_ascii_lowercase().as_str()))
}
fn map_key(rules: &RuleSet, key: &str) -> String {
    rules
        .mappings
        .iter()
        .find(|m| reader::same_key(&m.source_key, key))
        .map(|m| reader::canonical_key(&m.target_key))
        .unwrap_or_else(|| key.into())
}
fn preset_matches(file: &SourceFile, preset: &SheetPreset) -> bool {
    preset.workbook_sheets.is_empty()
        || preset
            .workbook_sheets
            .iter()
            .all(|name| file.sheets.iter().any(|s| s.name == *name))
}
fn check_cancel(cancel: &AtomicBool, started: Instant) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(AppError::new("CANCELLED", "任务已取消，未发布处理结果"))
    } else if started.elapsed().as_secs() > 3600 {
        Err(AppError::new("TIMEOUT", "任务超过 1 小时，已停止处理"))
    } else {
        Ok(())
    }
}

fn padded_identifier(cell:&Cell,width:Option<usize>)->Cell {
    if let (Cell::Number(n),Some(width))=(cell,width) {
        if *n>=0. && *n<=9_007_199_254_740_991. && n.fract()==0. {
            return Cell::Text(format!("{:0width$}",*n as u64));
        }
    }
    cell.clone()
}
fn activity(ws: &mut Workspace, action: &str, subject: &str) {
    ws.activities.insert(
        0,
        Activity {
            id: id(),
            action: action.into(),
            subject: subject.into(),
            timestamp: now(),
        },
    );
}
