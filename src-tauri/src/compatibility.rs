use crate::{engine::Engine, model::*};
use serde_json::Value;
pub fn convert(value: Value) -> Result<(RuleSet, Vec<SheetPreset>)> {
    if value
        .get("rules")
        .and_then(|r| r.get("version"))
        .and_then(Value::as_u64)
        == Some(2)
    {
        let scheme: Scheme = serde_json::from_value(value)?;
        return Ok((scheme.rules, scheme.sheets));
    }
    if value.get("version").and_then(Value::as_u64) == Some(2) {
        return Ok((serde_json::from_value(value)?, vec![]));
    }
    if !value.is_object()
        || ![
            "files",
            "file_configs",
            "cleaning",
            "cleaning_options",
            "columns",
        ]
        .iter()
        .any(|key| value.get(key).is_some())
    {
        return Err(AppError::new("CONFIG", "文件不是受支持的汇总配置"));
    }
    let mut rules = RuleSet::default();
    let cleaning = value
        .get("cleaning")
        .or(value.get("cleaning_options"))
        .unwrap_or(&value);
    let flag = |camel: &str, snake: &str| {
        cleaning
            .get(camel)
            .or(cleaning.get(snake))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    rules.clean_numeric = flag("cleanNumeric", "clean_numeric");
    rules.parse_dates = flag("parseDates", "parse_dates");
    rules.remove_empty_rows = flag("removeEmptyRows", "remove_empty_rows");
    rules.remove_duplicates = flag("removeDuplicates", "remove_duplicates");
    rules.merge_strategy = text(cleaning, "mergeStrategy", "merge_strategy", "outer");
    rules.keep_strategy = text(cleaning, "keepStrategy", "keep_strategy", "first");
    if let Some(columns) = value.get("columns").and_then(Value::as_array) {
        for col in columns {
            let key = text(col, "originalName", "key", "");
            if key.is_empty() {
                continue;
            }
            let ty = text(col, "columnType", "data_type", "auto");
            rules.columns.push(ColumnRule {
                key: key.clone(),
                name: text(col, "displayName", "name", &key),
                data_type: column_type(&ty),
                hidden: !col.get("visible").and_then(Value::as_bool).unwrap_or(true),
            });
        }
    }
    if let Some(types) = cleaning.get("column_types").and_then(Value::as_object) {
        for (key, ty) in types {
            if let Some(rule) = rules.columns.iter_mut().find(|c| c.key == *key) {
                rule.data_type = column_type(ty.as_str().unwrap_or("auto"));
            } else {
                rules.columns.push(ColumnRule {
                    key: key.clone(),
                    name: key.clone(),
                    data_type: column_type(ty.as_str().unwrap_or("auto")),
                    hidden: false,
                });
            }
        }
    }
    if let Some(names) = cleaning.get("column_names").and_then(Value::as_object) {
        for (key, name) in names {
            if let Some(rule) = rules.columns.iter_mut().find(|c| c.key == *key) {
                rule.name = name.as_str().unwrap_or(key).into();
            } else {
                rules.columns.push(ColumnRule {
                    key: key.clone(),
                    name: name.as_str().unwrap_or(key).into(),
                    data_type: ColumnType::Auto,
                    hidden: false,
                });
            }
        }
    }
    if let Some(hidden) = cleaning.get("hidden_columns").and_then(Value::as_array) {
        for col in &mut rules.columns {
            if hidden.iter().any(|v| v.as_str() == Some(&col.key)) {
                col.hidden = true;
            }
        }
    }
    if let Some(keys) = cleaning.get("duplicate_columns").and_then(Value::as_array) {
        rules.duplicate_columns = keys
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
    }
    let fixed = value
        .get("fixedCells")
        .and_then(|v| v.get("rules"))
        .or_else(|| cleaning.get("fixed_cells_rules"));
    if let Some(fixed) = fixed.and_then(Value::as_array) {
        for rule in fixed {
            let sheet_name = text(rule, "sheetName", "sheet_name", "");
            let address = text(rule, "cellAddress", "cell_address", "");
            let name = text(rule, "newColumnName", "column_name", "");
            if !sheet_name.is_empty() && !address.is_empty() {
                crate::reader::parse_address(&address)?;
                rules.fixed_cells.push(FixedCellRule {
                    id: id(),
                    name: if name.is_empty() {
                        format!("{sheet_name} {address}")
                    } else {
                        name
                    },
                    sheet_name,
                    address,
                });
            }
        }
    }
    if let Some(old) = value
        .get("cellExtraction")
        .and_then(|v| v.get("rules"))
        .and_then(Value::as_array)
    {
        for rule in old {
            let row = rule.get("rowNumber").and_then(Value::as_u64).unwrap_or(1) as usize;
            let col = rule.get("colNumber").and_then(Value::as_u64).unwrap_or(1) as usize;
            rules.fixed_cells.push(FixedCellRule {
                id: id(),
                name: text(rule, "newColumnName", "column_name", "固定值"),
                sheet_name: text(rule, "sheetName", "sheet_name", "Sheet1"),
                address: crate::reader::cell_address(row, col),
            });
        }
    }
    let mut presets = Vec::new();
    if let Some(files) = value.get("files").and_then(Value::as_array) {
        for file in files {
            if let Some(sheets) = file.get("sheets").and_then(Value::as_array) {
                for sheet in sheets {
                    let name = sheet
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| text(sheet, "name", "sheet_name", "Sheet1"));
                    let header_row =
                        sheet.get("headerRow").and_then(Value::as_u64).unwrap_or(1) as usize;
                    let enabled = file.get("enabled").and_then(Value::as_bool).unwrap_or(true)
                        && sheet
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                    if !presets.iter().any(|p: &SheetPreset| p.name == name) {
                        presets.push(SheetPreset {
                            name,
                            header_row,
                            enabled,
                            workbook_sheets: vec![],
                            header_range: None,
                        });
                    }
                }
            } else if let Some(name) = file.get("sheet").and_then(Value::as_str) {
                presets.push(SheetPreset {
                    name: name.into(),
                    header_row: file.get("headerRow").and_then(Value::as_u64).unwrap_or(1) as usize,
                    enabled: true,
                    workbook_sheets: vec![],
                    header_range: None,
                });
            }
        }
    }
    if let Some(sources) = value.get("file_configs").and_then(Value::as_array) {
        for source in sources {
            let name = text(source, "sheetName", "sheet_name", "Sheet1");
            let header_row = source
                .get("header_row")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize
                + 1;
            if !presets.iter().any(|p| p.name == name) {
                presets.push(SheetPreset {
                    name,
                    header_row,
                    enabled: true,
                    workbook_sheets: vec![],
                    header_range: None,
                });
            }
        }
    }
    Ok((rules, presets))
}
fn text(v: &Value, camel: &str, snake: &str, default: &str) -> String {
    v.get(camel)
        .or(v.get(snake))
        .and_then(Value::as_str)
        .unwrap_or(default)
        .into()
}
fn column_type(s: &str) -> ColumnType {
    match s {
        "number" | "numeric" => ColumnType::Number,
        "text" => ColumnType::Text,
        "date" => ColumnType::Date,
        "skip" => ColumnType::Skip,
        _ => ColumnType::Auto,
    }
}
impl Engine {
    pub fn import_scheme(&self, path: String) -> Result<Scheme> {
        self.guard()?;
        let bytes = std::fs::read(&path)?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err(AppError::new("CONFIG", "配置文件超过 5MB"));
        }
        let value = serde_json::from_slice::<Value>(&bytes)?;
        let (rules, sheets) = convert(value)?;
        // 同名覆盖只对手动保存生效；导入永远新增，否则两份同名导出文件会互相顶掉
        let mut name: String = std::path::Path::new(&path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        while self.snapshot()?.schemes.iter().any(|s| s.name == name) {
            name.push('·');
        }
        let scheme = self.save_scheme(name, rules)?;
        self.replace_scheme_sheets(&scheme.id, sheets)
    }
    pub fn export_scheme(&self, scheme_id: String, path: String) -> Result<()> {
        self.guard()?;
        let ws = self.snapshot()?;
        let scheme = ws
            .schemes
            .iter()
            .find(|s| s.id == scheme_id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "方案不存在"))?;
        std::fs::write(path, serde_json::to_vec_pretty(scheme)?)?;
        Ok(())
    }
    pub fn import_results(&self, paths: Vec<String>) -> Result<Vec<Dataset>> {
        let report = self.import_files(paths)?;
        let mut datasets = Vec::new();
        for item in report {
            let file = item
                .file
                .ok_or_else(|| AppError::new("IMPORT", item.error.unwrap_or_default()))?;
            let mut rules = RuleSet::default();
            rules.include_source = false;
            rules.sources = file
                .sheets
                .iter()
                .map(|s| SourceSelection {
                    sheet_id: s.id.clone(),
                    enabled: true,
                    header_row: 1,
                    header_range: None,
                    source_name: file.name.clone(),
                })
                .collect();
            let ds = self.process(
                &rules,
                false,
                &std::sync::atomic::AtomicBool::new(false),
                |_, _| {},
            )?;
            datasets.push(self.publish_imported(ds, &file.name)?);
        }
        Ok(datasets)
    }
}
