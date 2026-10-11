use crate::{
    analysis::{aggregate_query, conditions, order, visible},
    engine::{cell_from_row, Engine},
    model::*,
};
use duckdb::params_from_iter;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

impl Engine {
    pub fn export_dataset(&self, query: DatasetQuery, path: String) -> Result<String> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let ds = self.dataset(&query.dataset_id)?;
        let conn = self.conn()?;
        let indices = visible(&ds);
        let (condition, args) = conditions(&ds, &query)?;
        let selection = indices
            .iter()
            .flat_map(|i| [format!("c{i}"), format!("t{i}")])
            .collect::<Vec<_>>()
            .join(",");
        if selection.is_empty() {
            return Err(AppError::new("COLUMN", "没有可导出的列"));
        }
        let sql = format!(
            "SELECT {selection} FROM d_{}{condition}{}",
            ds.id,
            order(&ds, &query)?
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut cursor = stmt.query(params_from_iter(args))?;
        let columns = indices
            .iter()
            .map(|i| ds.columns[*i].clone())
            .collect::<Vec<_>>();
        let path = PathBuf::from(path);
        let temp = temporary(&path)?;
        let result = write_rows(&temp, &path, &columns, || match cursor.next() {
            Ok(Some(row)) => Some(
                (0..indices.len())
                    .map(|i| cell_from_row(row, 2 * i))
                    .collect::<Result<Vec<_>>>(),
            ),
            Ok(None) => None,
            Err(e) => Some(Err(e.into())),
        });
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
        for dataset_id in ids {
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
                    dataset_id,
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
        let sheet = book.add_worksheet_with_constant_memory();
        sheet.set_name("汇总数据").map_err(xlsx_error)?;
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
        book.save(temp).map_err(xlsx_error)?;
    } else {
        return Err(AppError::new("EXPORT", "请使用 .xlsx 或 .csv 扩展名"));
    }
    Ok(())
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
