use crate::{
    analysis::{aggregate_query, conditions, order, visible},
    engine::{cell_from_row, Engine},
    model::*,
};
use duckdb::{params_from_iter, arrow::{array::{Array, Int64Array}, record_batch::RecordBatch}};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
use std::{
    collections::HashMap,
    io::Write,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
};

impl Engine {
    pub fn export_workbook(&self, queries: Vec<DatasetQuery>, path: String) -> Result<String> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let path = PathBuf::from(path);
        if queries.is_empty() || !path.extension().is_some_and(|v| v.eq_ignore_ascii_case("xlsx")) {
            return Err(AppError::new("EXPORT", "请选择结果并保存为 .xlsx 文件"));
        }
        let temp = temporary(&path)?;
        let conn = self.conn()?;
        let mut names = std::collections::HashSet::from(["history".to_string()]);
        let mut exported = Vec::new();
        let result = (|| {
            let mut book = Workbook::new();
            for query in queries {
                let ds = self.dataset(&query.dataset_id)?;
                let indices = visible(&ds);
                if indices.is_empty() { return Err(AppError::new("COLUMN", format!("{} 没有可导出的列", ds.name))); }
                // Sort row IDs and keys, so DuckDB need not carry every exported text column through the sort.
                let sort_conn = sorted_export(&conn, &ds, &query)?;
                let mut statement = sort_conn.prepare("SELECT _row FROM export_order ORDER BY ordinal")?;
                let cursor = statement.stream_arrow([])?;
                let columns = indices.iter().map(|i| ds.columns[*i].clone()).collect::<Vec<_>>();
                let name = worksheet_name(&ds.name, &mut names);
                write_worksheet(&mut book, &name, &columns, export_rows(&conn, cursor, &ds.id, &indices))?;
                exported.push(ds.id);
            }
            book.save(&temp).map_err(xlsx_error)?;
            publish(&temp, &path)
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
        for id in exported { self.record_export(&id, &path)?; }
        Ok(path.to_string_lossy().into())
    }
    pub fn export_dataset(&self, query: DatasetQuery, path: String) -> Result<String> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let ds = self.dataset(&query.dataset_id)?;
        let conn = self.conn()?;
        let indices = visible(&ds);
        if indices.is_empty() {
            return Err(AppError::new("COLUMN", "没有可导出的列"));
        }
        let sort_conn = sorted_export(&conn, &ds, &query)?;
        let mut stmt = sort_conn.prepare("SELECT _row FROM export_order ORDER BY ordinal")?;
        let cursor = stmt.stream_arrow([])?;
        let columns = indices
            .iter()
            .map(|i| ds.columns[*i].clone())
            .collect::<Vec<_>>();
        let path = PathBuf::from(path);
        let temp = temporary(&path)?;
        let result = write_rows(&temp, &path, &columns, export_rows(&conn, cursor, &ds.id, &indices));
        match result {
            Ok(()) => {
                publish(&temp, &path)?;
                self.record_export(&ds.id, &path)?;
                Ok(path.to_string_lossy().into())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                Err(e)
            }
        }
    }
    pub fn export_analysis(&self, spec: PivotSpec, path: String) -> Result<String> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let ds = self.dataset(&spec.query.dataset_id)?;
        let conn = self.conn()?;
        let analysis = aggregate_query(&conn, &ds, spec, None)?;
        let path = PathBuf::from(path);
        let temp = temporary(&path)?;
        let mut rows = analysis.rows.into_iter();
        let result = write_rows(&temp, &path, &analysis.columns, || rows.next().map(Ok));
        match result {
            Ok(()) => {
                publish(&temp, &path)?;
                self.record_export(&ds.id, &path)?;
                Ok(path.to_string_lossy().into())
            }
            Err(e) => {
                let _ = std::fs::remove_file(temp);
                Err(e)
            }
        }
    }
    pub fn export_batch(
        &self,
        ids: Vec<String>,
        directory: String,
        zip_path: Option<String>,
        format: String,
    ) -> Result<Vec<String>> {
        self.guard()?;
        if ids.is_empty() {
            return Err(AppError::new("EXPORT", "请先选择结果"));
        }
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory)?;
        let mut paths = Vec::new();
        for dataset_id in &ids {
            let ds = self.dataset(&dataset_id)?;
            let name = format!(
                "{}_{}.{}",
                safe_name(&ds.name),
                &ds.id[..8],
                if format == "csv" { "csv" } else { "xlsx" }
            );
            let path = directory.join(name);
            paths.push(self.export_dataset(
                DatasetQuery {
                    dataset_id: dataset_id.clone(),
                    limit: 100,
                    ..Default::default()
                },
                path.to_string_lossy().into(),
            )?);
        }
        if let Some(zip_path) = zip_path {
            let path = PathBuf::from(&zip_path);
            let temp = temporary(&path)?;
            let result = (|| {
                let file = std::fs::File::create(&temp)?;
                let mut writer = zip::ZipWriter::new(file);
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                for item in &paths {
                    let p = Path::new(item);
                    writer
                        .start_file(p.file_name().unwrap().to_string_lossy(), options)
                        .map_err(|e| AppError::new("ZIP", e.to_string()))?;
                    let mut file = std::fs::File::open(p)?;
                    std::io::copy(&mut file, &mut writer)?;
                }
                writer
                    .finish()
                    .map_err(|e| AppError::new("ZIP", e.to_string()))?;
                publish(&temp, &path)?;
                Ok::<_, AppError>(())
            })();
            if let Err(error) = result {
                let _ = std::fs::remove_file(temp);
                return Err(error);
            }
            for id in &ids { self.record_export(id, &path)?; }
            paths.push(zip_path);
        }
        Ok(paths)
    }
    pub fn save_image(&self, path: String, bytes: Vec<u8>) -> Result<String> {
        self.guard()?;
        if !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
            return Err(AppError::new("IMAGE", "图表数据不是 PNG 图片"));
        }
        let path = PathBuf::from(path);
        let temp = temporary(&path)?;
        std::fs::write(&temp, bytes)?;
        publish(&temp, &path)?;
        Ok(path.to_string_lossy().into())
    }
}
fn sorted_export(conn: &duckdb::Connection, ds: &Dataset, query: &DatasetQuery) -> Result<duckdb::Connection> {
    let sorted = conn.try_clone()?;
    let (condition, args) = export_conditions(ds, query)?;
    // A private connection owns this temporary table and drops it on every success/error path.
    // Finish filtering/sorting before fetching wide values, releasing those operators' working memory.
    sorted.execute(&format!(
        "CREATE TEMP TABLE export_order AS SELECT _row,ROW_NUMBER() OVER ({}) AS ordinal FROM d_{}{condition}",
        order(ds, query)?, ds.id
    ), params_from_iter(args))?;
    Ok(sorted)
}
fn export_conditions(ds: &Dataset, query: &DatasetQuery) -> Result<(String, Vec<duckdb::types::Value>)> {
    let (condition, args) = conditions(ds, query)?;
    if query.search.trim().is_empty() { return Ok((condition, args)); }
    let mut without_search = query.clone();
    without_search.search.clear();
    let (base, mut filter_args) = conditions(ds, &without_search)?;
    let filter_count = filter_args.len();
    // The same escaped search predicates and hidden columns, scanned separately to avoid a wide OR scan.
    let matches = (0..ds.columns.len()).map(|i| format!(
        "SELECT _row FROM d_{} WHERE LOWER(COALESCE(c{i},'')) LIKE ? ESCAPE '$'", ds.id
    )).collect::<Vec<_>>().join(" UNION ");
    filter_args.extend(args.into_iter().skip(filter_count));
    Ok((format!("{base}{} _row IN ({matches})", if base.is_empty() { " WHERE" } else { " AND" }), filter_args))
}
fn export_rows<'a>(conn: &'a duckdb::Connection, mut batches: duckdb::ArrowStream<'a>, dataset_id: &str, indices: &[usize]) -> impl FnMut() -> Option<Result<Vec<Cell>>> + 'a {
    let table = format!("d_{dataset_id}");
    let width = indices.len();
    let indices = indices.to_vec();
    let batch_size = (16_384 / width.max(1)).clamp(1, 2048);
    let mut batch: Option<RecordBatch> = None;
    let mut row = 0;
    let mut failed = false;
    let mut buffered = Vec::new().into_iter();
    move || {
        if failed { return None; }
        if let Some(cells) = buffered.next() { return Some(Ok(cells)); }
        while batch.as_ref().is_none_or(|batch| row >= batch.num_rows()) {
            // DuckDB's Arrow iterator panics on fetch errors; preserve normal export failure and temp-file cleanup.
            match catch_unwind(AssertUnwindSafe(|| batches.next())) {
                Ok(Some(next)) => { batch = Some(next); row = 0; }
                Ok(None) => return None,
                Err(error) => {
                    failed = true;
                    let message = error.downcast_ref::<String>().cloned()
                        .or_else(|| error.downcast_ref::<&str>().map(|s| (*s).to_string()))
                        .unwrap_or_else(|| "读取导出数据失败".into());
                    return Some(Err(AppError::new("DATABASE", message)));
                }
            }
        }
        let result = (|| -> Result<Vec<Vec<Cell>>> {
            let batch = batch.as_ref().unwrap();
            let ids = batch.column(0).as_any().downcast_ref::<Int64Array>()
                .ok_or_else(|| AppError::new("DATABASE", "导出行号类型不匹配"))?;
            let end = (row + batch_size).min(batch.num_rows());
            let mut values = Vec::with_capacity(end - row);
            for index in row..end {
                if ids.is_null(index) { return Err(AppError::new("DATABASE", "导出行号为空")); }
                values.push(ids.value(index));
            }
            row = end;
            // Filter before fetching wide values, then restore the exact order from the sorted row-ID stream.
            let row_list = values.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            let mut rows = HashMap::with_capacity(values.len());
            // Bound scan width too: one DuckDB vector of many long text columns can exceed a small budget.
            for (chunk, columns) in indices.chunks(8).enumerate() {
                let selection = columns.iter().flat_map(|i| [format!("c{i}"), format!("t{i}")]).collect::<Vec<_>>().join(",");
                let mut statement = conn.prepare(&format!("SELECT _row,{selection} FROM {table} WHERE _row IN ({row_list})"))?;
                let mut cursor = statement.query([])
                    .map_err(|e| AppError::new("DATABASE", format!("导出批次读取失败：{e}")))?;
                let mut fetched = 0;
                while let Some(record) = cursor.next()? {
                    let cells = rows.entry(record.get::<_, i64>(0)?).or_insert_with(|| vec![Cell::Empty; width]);
                    for i in 0..columns.len() { cells[chunk * 8 + i] = cell_from_row(record, 1 + 2 * i)?; }
                    fetched += 1;
                }
                if fetched != values.len() { return Err(AppError::new("DATABASE", "导出数据读取不完整，请重试")); }
            }
            if rows.len() != values.len() { return Err(AppError::new("DATABASE", "导出数据读取不完整，请重试")); }
            values.into_iter().map(|id| rows.remove(&id).ok_or_else(|| AppError::new("DATABASE", "导出行号不存在，请重试"))).collect()
        })();
        match result {
            Ok(rows) => { buffered = rows.into_iter(); buffered.next().map(Ok) }
            Err(error) => { failed = true; Some(Err(error)) }
        }
    }
}
fn write_rows(
    temp: &Path,
    path: &Path,
    columns: &[Column],
    mut next: impl FnMut() -> Option<Result<Vec<Cell>>>,
) -> Result<()> {
    let format = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if format == "csv" {
        let mut file = std::fs::File::create(temp)?;
        file.write_all(&[0xef, 0xbb, 0xbf])?;
        let mut writer = csv::Writer::from_writer(file);
        writer
            .write_record(columns.iter().map(|c| c.name.as_str()))
            .map_err(|e| AppError::new("EXPORT", e.to_string()))?;
        while let Some(row) = next() {
            writer
                .write_record(row?.iter().map(|cell| csv_text(cell)))
                .map_err(|e| AppError::new("EXPORT", e.to_string()))?;
        }
        writer.flush()?;
    } else if format == "xlsx" {
        let mut book = Workbook::new();
        write_worksheet(&mut book, "汇总数据", columns, next)?;
        book.save(temp).map_err(xlsx_error)?;
    } else {
        return Err(AppError::new("EXPORT", "请使用 .xlsx 或 .csv 扩展名"));
    }
    Ok(())
}
fn write_worksheet(
    book: &mut Workbook,
    name: &str,
    columns: &[Column],
    mut next: impl FnMut() -> Option<Result<Vec<Cell>>>,
) -> Result<()> {
    if columns.len() > 16_384 { return Err(AppError::new("EXCEL_LIMIT", "Excel 超过最大列数，请减少输出列")); }
    let sheet = book.add_worksheet_with_constant_memory();
    sheet.set_name(name).map_err(xlsx_error)?;
    sheet.set_freeze_panes(1, 0).map_err(xlsx_error)?;
    let header = Format::new().set_bold().set_background_color("#EFF6FF");
    let date_format = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
    for (i, column) in columns.iter().enumerate() {
        sheet
            .write_string_with_format(0, i as u16, &column.name, &header)
            .map_err(xlsx_error)?;
        sheet.set_column_width(i as u16, 18).map_err(xlsx_error)?;
    }
    let mut number = 1u32;
    while let Some(row) = next() {
        if number >= 1_048_576 {
            return Err(AppError::new(
                "EXCEL_LIMIT",
                "Excel 超过最大行数，请导出 CSV",
            ));
        }
        for (i, cell) in row?.iter().enumerate() {
            match cell {
                Cell::Empty => {}
                Cell::Text(text) => {
                    sheet
                        .write_string(number, i as u16, text)
                        .map_err(xlsx_error)?;
                }
                Cell::Number(value) => {
                    sheet
                        .write_number(number, i as u16, *value)
                        .map_err(xlsx_error)?;
                }
                Cell::Bool(value) => {
                    sheet
                        .write_boolean(number, i as u16, *value)
                        .map_err(xlsx_error)?;
                }
                Cell::Date(value) => {
                    let formatted = if value.len() == 10 {
                        format!("{value}T00:00:00")
                    } else {
                        value.replace(' ', "T")
                    };
                    if let Ok(date) = ExcelDateTime::parse_from_str(&formatted) {
                        sheet
                            .write_datetime_with_format(number, i as u16, &date, &date_format)
                            .map_err(xlsx_error)?;
                    } else {
                        sheet
                            .write_string(number, i as u16, value)
                            .map_err(xlsx_error)?;
                    }
                }
            }
        }
        number += 1;
    }
    if number > 1 && !columns.is_empty() {
        sheet
            .autofilter(0, 0, number - 1, columns.len() as u16 - 1)
            .map_err(xlsx_error)?;
    }
    Ok(())
}
fn worksheet_name(name: &str, used: &mut std::collections::HashSet<String>) -> String {
    let base = name.chars().map(|c| if "[]:*?/\\".contains(c) || c.is_control() { '_' } else { c })
        .collect::<String>().trim().trim_matches('\'').to_string();
    let base = if base.is_empty() { "汇总数据" } else { &base };
    let mut number = 1;
    loop {
        let suffix = if number == 1 { String::new() } else { format!(" ({number})") };
        let candidate = format!("{}{}", base.chars().take(31 - suffix.chars().count()).collect::<String>(), suffix);
        if used.insert(candidate.to_lowercase()) { return candidate; }
        number += 1;
    }
}
fn csv_text(cell: &Cell) -> String {
    let text = cell.text();
    if matches!(cell, Cell::Text(_)) && text.trim_start().starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text
    }
}
fn xlsx_error(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::new("EXPORT", e.to_string())
}
fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}
fn temporary(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("EXPORT", "导出路径无效"))?;
    if !parent.is_dir() {
        return Err(AppError::new("EXPORT", "导出目录不存在"));
    }
    Ok(parent.join(format!(".workbench-{}.tmp", id())))
}
fn publish(temp: &Path, path: &Path) -> Result<()> {
    // 保留原文件直到临时文件已完整写出；失败时回滚原文件。
    let backup = path.with_extension(format!("backup-{}", id()));
    let existed = path.exists();
    if existed {
        std::fs::rename(path, &backup)?;
    }
    if let Err(error) = std::fs::rename(temp, path) {
        if existed {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    if existed {
        std::fs::remove_file(backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn late_stream_error_is_returned_instead_of_truncating_export_rows() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch("SET threads=1; CREATE TABLE d_late AS SELECT i::BIGINT AS _row,i::VARCHAR AS c0,1::UTINYINT AS t0 FROM range(1,10001) input(i)").unwrap();
        let sorted = conn.try_clone().unwrap();
        let interrupt = sorted.interrupt_handle();
        let mut statement = sorted.prepare("SELECT i::BIGINT AS _row FROM range(1,1000000000) input(i)").unwrap();
        let stream = statement.stream_arrow([]).unwrap();
        let mut next = super::export_rows(&conn, stream, "late", &[0]);
        assert!(next().unwrap().is_ok());
        interrupt.interrupt();
        let mut count = 1;
        loop {
            match next().expect("fetch failure must not become end-of-file") {
                Ok(_) => count += 1,
                Err(error) => { assert!(error.message.contains("Interrupted"), "{error:?}"); break; }
            }
        }
        assert!(count > 0, "exercise a failure after exporting a completed batch");
        assert!(next().is_none());
    }
}
