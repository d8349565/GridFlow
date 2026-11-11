use crate::model::*;
use calamine::{open_workbook_auto, Cell as ExcelCell, Data, DataRef, Range, Reader, Sheets, XlsxError};
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

pub struct ReadSheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
    pub width: usize,
    pub note: Option<String>,
}

struct TableExtent { area: SheetRange, header_end: usize, data_end: usize, read_end: usize }
impl TableExtent {
    fn contains(&self, (row, column): (u32, u32)) -> bool {
        let (row, column) = (row as usize + 1, column as usize + 1);
        row >= self.area.start_row && row <= self.area.end_row && column >= self.area.start_column && column <= self.area.end_column
    }
    fn header(&self, position: (u32, u32)) -> bool { self.contains(position) && position.0 as usize + 1 <= self.header_end }
    fn include_data(&mut self, position: (u32, u32)) { if self.contains(position) { self.data_end = self.data_end.max(position.1 as usize + 1); } }
}
fn table_extents(path: &Path, sheet_name: &str) -> Result<Vec<TableExtent>> {
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|e| AppError::new("TABLE", e.to_string()))?;
    let workbook = crate::appearance::xml(&mut zip, "xl/workbook.xml")?;
    let sheet = crate::appearance::sheet_path(&mut zip, &workbook, sheet_name)?;
    let (folder, file) = sheet.rsplit_once('/').unwrap_or(("", sheet.as_str()));
    let relationships = crate::appearance::xml(&mut zip, &format!("{folder}/_rels/{file}.rels"))?;
    let mut parts = Vec::new();
    crate::appearance::visit(&relationships, |tag,a,end| {
        if !end && tag == "Relationship" && a.get("Type").is_some_and(|v| v.ends_with("/table")) && a.get("TargetMode").is_none_or(|v| v != "External") {
            if let Some(target) = a.get("Target") { parts.push(crate::appearance::resolve_part(&sheet, target)); }
        }
    })?;
    let mut tables = Vec::new();
    for part in parts {
        crate::appearance::visit(&crate::appearance::xml(&mut zip, &part)?, |tag, a, end| {
            if tag != "table" || end { return; }
            let Some(area) = a.get("ref").and_then(|v| crate::appearance::range(v)) else { return; };
            let headers = a.get("headerRowCount").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
            if headers == 0 || headers > area.end_row - area.start_row + 1 { return; }
            tables.push(TableExtent { header_end: area.start_row + headers - 1, data_end: area.start_column, read_end: area.end_column, area });
        })?;
    }
    Ok(tables)
}
pub struct ReadFile {
    pub sheets: Vec<ReadSheet>,
    pub encoding: Option<String>,
}

fn excel_bounds(size: &mut (usize, usize), position: (u32, u32), max_cells: usize) -> Result<()> {
    size.0 = size.0.max(position.0 as usize + 1);
    size.1 = size.1.max(position.1 as usize + 1);
    if size.1 > 2048 || size.0.saturating_mul(size.1) > max_cells {
        return Err(AppError::new("LIMIT", "工作表有效内容超过 2048 列或可用单元格上限（已忽略尾部空白），请缩小实际数据范围"));
    }
    Ok(())
}

/// Filter empty tails before constructing a dense range: a distant blank cell must not allocate the whole Excel grid.
fn excel_sheet(book: &mut Sheets<BufReader<File>>, path: &Path, name: &str, max_cells: usize, check: &impl Fn() -> Result<()>) -> Result<(Range<Data>, Range<String>, Option<String>)> {
    let error = |e| AppError::new("SHEET", format!("无法读取工作表“{name}”：{e}"));
    let mut size = (0, 0);
    if let Sheets::Xlsx(xlsx) = book {
        // Table metadata is optional. If it cannot be verified, retain headers and keep the normal safety limits.
        let mut tables = table_extents(path, name).unwrap_or_default();
        let mut formulas = Vec::new();
        {
            let mut cells = match xlsx.worksheet_cells_reader(name) {
                Ok(reader) => reader,
                Err(XlsxError::NotAWorksheet(_)) => return Ok((Range::empty(), Range::empty(), None)),
                Err(e) => return Err(error(e)),
            };
            let mut count = 0usize;
            while let Some(cell) = cells.next_formula().map_err(error)? {
                if count % 1024 == 0 { check()?; } count += 1;
                if !cell.get_value().is_empty() {
                    for table in &mut tables { table.include_data(cell.get_position()); }
                    excel_bounds(&mut size, cell.get_position(), max_cells)?; formulas.push(cell);
                }
            }
        }
        let formulas = Range::from_sparse(formulas);
        let mut values = Vec::new();
        let mut headers = Vec::new();
        let mut cells = xlsx.worksheet_cells_reader(name).map_err(error)?;
        let mut count = 0usize;
        while let Some(cell) = cells.next_cell().map_err(error)? {
            if count % 1024 == 0 { check()?; } count += 1;
            let position = cell.get_position();
            let blank = match cell.get_value() { DataRef::Empty => true, DataRef::String(v) => v.trim().is_empty(), DataRef::SharedString(v) => v.trim().is_empty(), _ => false };
            // A cached empty formula result differs from a formula with no saved result.
            let cached_formula = !matches!(cell.get_value(), DataRef::Empty) && formulas.get_value(position).is_some_and(|v| !v.is_empty());
            if !blank || cached_formula {
                if !cached_formula {
                    if let Some(index) = tables.iter().position(|t| t.header(position)) {
                        headers.push((index, ExcelCell::new(position, Data::from(cell.get_value().clone())))); continue;
                    }
                }
                for table in &mut tables { table.include_data(position); }
                excel_bounds(&mut size, position, max_cells)?;
                values.push(ExcelCell::new(position, Data::from(cell.get_value().clone())));
            }
        }
        let mut ignored = 0;
        for table in &mut tables {
            if table.area.end_column > 2048 || size.0.max(table.header_end).saturating_mul(table.area.end_column) > max_cells {
                table.read_end = table.data_end;
                ignored += table.area.end_column - table.read_end;
            }
        }
        for (index, cell) in headers {
            if cell.get_position().1 as usize + 1 <= tables[index].read_end {
                excel_bounds(&mut size, cell.get_position(), max_cells)?; values.push(cell);
            }
        }
        let note = (ignored > 0).then(|| format!("超级表尾部 {ignored} 列仅含表头、下方无数据，已限制读取范围；原文件未修改。"));
        return Ok((Range::from_sparse(values), formulas, note));
    }
    let values = book.worksheet_range(name).map_err(|e| AppError::new("SHEET", format!("无法读取工作表“{name}”：{e}")))?;
    check()?;
    let formulas = book.worksheet_formula(name).map_err(|e| AppError::new("SHEET", format!("无法读取工作表“{name}”的公式：{e}")))?;
    let origin = values.start().unwrap_or((0, 0));
    let mut bounds: Option<((u32, u32), (u32, u32))> = None;
    for (r, c, value) in values.used_cells() {
        let position = (origin.0 + r as u32, origin.1 + c as u32);
        if matches!(value, Data::String(v) if v.trim().is_empty()) && !formulas.get_value(position).is_some_and(|v| !v.is_empty()) { continue; }
        excel_bounds(&mut size, position, max_cells)?;
        let (start, end) = bounds.get_or_insert((position, position));
        start.0 = start.0.min(position.0); start.1 = start.1.min(position.1); end.0 = end.0.max(position.0); end.1 = end.1.max(position.1);
    }
    if let Some(end) = formulas.end() { excel_bounds(&mut size, end, max_cells)?; }
    let values = match bounds {
        Some((start, end)) if Some(start) == values.start() && Some(end) == values.end() => values,
        Some((start, end)) => values.range(start, end),
        None => Range::empty(),
    };
    Ok((values, formulas, None))
}

pub fn read(path: &Path) -> Result<ReadFile> {
    let mut sheets = Vec::new();
    let encoding = read_each(path, &|| Ok(()), |sheet| { sheets.push(sheet); Ok(()) })?;
    Ok(ReadFile { sheets, encoding })
}

/// Import consumes each sheet before parsing the next one; callers that need a complete workbook can use `read`.
pub(crate) fn read_each(path: &Path, check: &impl Fn() -> Result<()>, mut consume: impl FnMut(ReadSheet) -> Result<()>) -> Result<Option<String>> {
    check()?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match extension.as_str() {
        "xlsx" | "xls" => {
            let mut book = open_workbook_auto(path)
                .map_err(|e| AppError::new("EXCEL", format!("Excel 无法读取（可能已损坏或加密）：{e}")))?;
            let names = book.sheet_names().to_owned();
            if names.is_empty() { return Err(AppError::new("EMPTY", "工作簿没有工作表")); }
            for name in names {
                check()?;
                consume(read_excel_sheet(&mut book, path, name, check)?)?;
            }
            Ok(None)
        }
        "csv" => {
            let read = read_csv(path, check)?;
            for sheet in read.sheets { check()?; consume(sheet)?; }
            Ok(read.encoding)
        }
        _ => Err(AppError::new(
            "UNSUPPORTED",
            "请选择 .xlsx、.xls 或 .csv 文件",
        )),
    }
}
fn read_excel_sheet(book: &mut Sheets<BufReader<File>>, path: &Path, name: String, check: &impl Fn() -> Result<()>) -> Result<ReadSheet> {
        let (mut range, formulas, note) = excel_sheet(book, path, &name, 20_000_000, check)?;
        let ends = [range.end(), formulas.end()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let height = ends.iter().map(|p| p.0 as usize + 1).max().unwrap_or(0);
        let width = ends.iter().map(|p| p.1 as usize + 1).max().unwrap_or(0);
        let start = range.start().unwrap_or((0, 0));
        let end = range.end();
        let mut rows = Vec::with_capacity(height);
        for r in 0..height {
            check()?;
            let mut cells = Vec::with_capacity(width);
            for c in 0..width {
                let position = (r as u32, c as u32);
                let value = if end.is_some_and(|end| position.0 >= start.0 && position.1 >= start.1 && position.0 <= end.0 && position.1 <= end.1) {
                    std::mem::take(&mut range[(r - start.0 as usize, c - start.1 as usize)])
                } else { Data::Empty };
                let cell = match value {
                    Data::Empty => {
                        let has_formula = formulas
                            .get_value((r as u32, c as u32))
                            .is_some_and(|v| !v.is_empty());
                        if has_formula {
                            Cell::Text("#公式无缓存".into())
                        } else {
                            Cell::Empty
                        }
                    }
                    Data::String(v) => Cell::Text(v),
                    Data::Int(v) => Cell::Number(v as f64),
                    Data::Float(v) => Cell::Number(v),
                    Data::Bool(v) => Cell::Bool(v),
                    Data::DateTime(v) => v
                        .as_datetime()
                        .map(|d| Cell::Date(d.format("%Y-%m-%d %H:%M:%S").to_string()))
                        .unwrap_or(Cell::Number(v.as_f64())),
                    Data::DateTimeIso(v) => Cell::Date(v),
                    Data::DurationIso(v) => Cell::Text(v),
                    Data::Error(v) => Cell::Text(format!("#Excel错误:{v:?}")),
                };
                cells.push(cell);
            }
            rows.push(cells);
        }

        Ok(ReadSheet { name, rows, width, note })
}
fn read_csv(path: &Path, check: &impl Fn() -> Result<()>) -> Result<ReadFile> {
    let bytes = std::fs::read(path)?;
    let (text, encoding) = if bytes.starts_with(&[0xff, 0xfe]) {
        let (s, _, err) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        if err {
            return Err(AppError::new("ENCODING", "UTF-16 文件包含无效编码"));
        }
        (s.into_owned(), "UTF-16LE".into())
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        let (s, _, err) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        if err {
            return Err(AppError::new("ENCODING", "UTF-16 文件包含无效编码"));
        }
        (s.into_owned(), "UTF-16BE".into())
    } else if let Ok(s) = std::str::from_utf8(&bytes) {
        (s.trim_start_matches('\u{feff}').to_string(), "UTF-8".into())
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(&bytes, true);
        let enc = detector.guess(Some(b"cn"), true);
        let (s, _, err) = enc.decode(&bytes);
        if err {
            return Err(AppError::new(
                "ENCODING",
                "无法无损读取 CSV 编码，请转换为 UTF-8 后重试",
            ));
        }
        (s.into_owned(), enc.name().into())
    };
    drop(bytes);
    check()?;
    let delimiters = [b',', b';', b'\t'];
    let delimiter = delimiters
        .into_iter()
        .max_by_key(|d| {
            csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(*d)
                .flexible(true)
                .from_reader(text.as_bytes())
                .records()
                .take(5)
                .filter_map(|r| r.ok())
                .map(|r| r.len())
                .sum::<usize>()
        })
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    let mut width = 0;
    for row in reader.records() {
        if rows.len() % 256 == 0 { check()?; }
        let row = row.map_err(|e| AppError::new("CSV", e.to_string()))?;
        width = width.max(row.len());
        rows.push(
            row.iter()
                .map(|s| {
                    if s.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Text(s.into())
                    }
                })
                .collect(),
        );
    }
    if width > 2048 {
        return Err(AppError::new("LIMIT", "CSV 超过 2048 列"));
    }
    Ok(ReadFile {
        sheets: vec![ReadSheet {
            name: "CSV".into(),
            rows,
            width,
            note: None,
        }],
        encoding: Some(encoding),
    })
}

pub fn header_names(row: &[Cell], width: usize) -> Vec<String> {
    let mut seen = HashMap::new();
    let mut used = std::collections::HashSet::new();
    (0..width)
        .map(|i| {
            let name = row
                .get(i)
                .map(Cell::text)
                .unwrap_or_default()
                .trim()
                .to_string();
            let name = if name.is_empty() {
                format!("列{}", i + 1)
            } else {
                name
            };
            let count = seen.entry(name.clone()).or_insert(0);
            *count += 1;
            let mut output = if *count > 1 {
                format!("{name} #{}", count)
            } else {
                name.clone()
            };
            while !used.insert(output.clone()) {
                *count += 1;
                output = format!("{name} #{}", count);
            }
            output
        })
        .collect()
}
pub fn business_key(name: &str) -> String {
    format!("input:{name}")
}
pub fn canonical_key(key: &str) -> String {
    if key == "__source" || key.starts_with("fixed:") || key.starts_with("lookup:") || key.starts_with("input:") {
        key.into()
    } else {
        business_key(key)
    }
}
pub fn same_key(a: &str, b: &str) -> bool {
    canonical_key(a) == canonical_key(b)
}
pub fn field_label(key: &str) -> String {
    key.strip_prefix("input:").unwrap_or(key).into()
}
pub fn suggest_header(rows: &[Vec<Cell>]) -> usize {
    let score = |row: &Vec<Cell>| -> (i64, i64) {
        let filled = row.iter().filter(|c| !c.text().trim().is_empty()).count() as i64;
        let texts = row
            .iter()
            .filter(|c| matches!(c, Cell::Text(s) if !s.trim().is_empty()))
            .count() as i64;
        // 含有冒号结尾的标签（如「填写人:」、「申报日期:」）是典型的表单头元数据行，非母表列名，予以重度降权
        let colons = row
            .iter()
            .filter(|c| {
                let text = c.text();
                let t = text.trim();
                t.ends_with(':') || t.ends_with('：')
            })
            .count() as i64;
        let penalty = colons * 10;
        (filled.min(texts * 2) - penalty, texts - penalty)
    };
    rows.iter()
        .take(20)
        .enumerate()
        .max_by(|(ia, a), (ib, b)| score(a).cmp(&score(b)).then_with(|| ib.cmp(ia)))
        .map(|(i, _)| i + 1)
        .unwrap_or(1)
}
pub fn suggest_type(values: &[Cell]) -> ColumnType {
    let values: Vec<_> = values
        .iter()
        .filter(|c| !c.text().trim().is_empty())
        .take(100)
        .collect();
    if values.is_empty() {
        return ColumnType::Text;
    }
    // 编号的前导零是数据，类型建议也不能将它当成可删除的格式。
    if values.iter().any(|v|matches!(v,Cell::Text(s) if s.trim().chars().all(|c|c.is_ascii_digit()) && ((s.trim().starts_with('0') && s.trim().len()>1)||s.trim().len()>15))) {return ColumnType::Text;}
    if matches_type_threshold(&values, |value| matches!(value, Cell::Date(_)) || parse_date(&value.text()).is_some()) {
        return ColumnType::Date;
    }
    if matches_type_threshold(&values, |value| parse_number(&value.text()).is_some()) {
        return ColumnType::Number;
    }
    ColumnType::Text
}
fn matches_type_threshold(values: &[&Cell], is_match: impl Fn(&Cell) -> bool) -> bool {
    let required = (values.len() * 4).div_ceil(5);
    let mut matches = 0;
    for (index, value) in values.iter().enumerate() {
        matches += usize::from(is_match(value));
        if matches >= required { return true; }
        if matches + values.len() - index - 1 < required { return false; }
    }
    false
}
pub fn suggest_column_type(name: &str, values: &[Cell]) -> ColumnType {
    let trimmed = name.trim();
    if trimmed.ends_with(':') || trimmed.ends_with('：') {
        return ColumnType::Text;
    }
    let lower = trimmed.to_ascii_lowercase();
    if [
        "编号",
        "编码",
        "代码",
        "手机",
        "电话",
        "邮编",
        "身份证",
        "单号",
        "账号",
        "卡号",
        "序号",
        "工号",
        "批号",
        "税号",
    ]
    .iter()
    .any(|s| trimmed.contains(s))
        || lower == "id"
        || lower.ends_with("_id")
        || lower == "sku"
    {
        return ColumnType::Text;
    }

    let ty = suggest_type(values);
    let is_date_name = [
        "日期", "时间", "月份", "年份", "年月", "季度", "date", "time", "month", "year",
    ]
    .iter()
    .any(|s| lower.contains(s));

    // 语义护栏：日期类列名绝不能建议为数字
    if matches!(ty, ColumnType::Number) && is_date_name {
        return ColumnType::Text;
    }
    // 语义护栏：金额/费用/数量列绝不能建议为日期（除非名称本身包含明确的日期/时间语义）
    if matches!(ty, ColumnType::Date)
        && !is_date_name
        && ([
            "金额", "费用", "价格", "单价", "成本", "数量", "比例", "率", "amount", "price",
            "count", "qty", "cost", "fee", "rate", "sum", "total",
        ]
        .iter()
        .any(|s| lower.contains(s)))
    {
        return ColumnType::Text;
    }

    ty
}
pub fn convert_cell(
    cell: &Cell,
    ty: &ColumnType,
    date_format: &str,
) -> std::result::Result<Cell, String> {
    if matches!(cell, Cell::Empty) {
        return Ok(Cell::Empty);
    }
    match ty {
        ColumnType::Text => Ok(Cell::Text(cell.text())),
        ColumnType::Number => parse_number(&cell.text())
            .map(Cell::Number)
            .ok_or_else(|| "无法转换为数值，保留原值".into()),
        ColumnType::Date => parse_date_with_format(&cell.text(), date_format)
            .map(Cell::Date)
            .ok_or_else(|| "无法确定日期格式，保留原值；可选择日月年或月日年".into()),
        _ => Ok(cell.clone()),
    }
}
pub fn parse_number(text: &str) -> Option<f64> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    let percent = s.ends_with('%');
    let mut s = s
        .replace(['￥', '¥', '$', '€', '£', ' ', '\u{a0}'], "")
        .trim_end_matches('%')
        .to_string();
    let negative = s.starts_with('(') && s.ends_with(')');
    if negative {
        s = s[1..s.len() - 1].to_string();
    }
    if let (Some(comma), Some(dot)) = (s.rfind(','), s.rfind('.')) {
        if comma > dot {
            s = s.replace('.', "").replace(',', ".")
        } else {
            s = s.replace(',', "")
        }
    } else {
        s = s.replace(',', "");
    }
    let mut value = s.parse::<f64>().ok()?;
    if !value.is_finite() {
        return None;
    }
    if negative {
        value = -value;
    }
    if percent {
        value /= 100.;
    }
    Some(value)
}
pub fn parse_date(text: &str) -> Option<String> {
    parse_date_with_format(text, "auto")
}
pub fn parse_date_with_format(text: &str, mode: &str) -> Option<String> {
    if text.len() < 8 {
        return None;
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%Y%m%d", "%Y年%m月%d日", "%d.%m.%Y"] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(text.trim(), fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(text.trim(), fmt) {
            return Some(d.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }
    let formats = match mode {
        "dmy" => vec!["%d/%m/%Y", "%d-%m-%Y"],
        "mdy" => vec!["%m/%d/%Y", "%m-%d-%Y"],
        "ymd" => vec![],
        _ => vec!["%d/%m/%Y", "%d-%m-%Y", "%m/%d/%Y", "%m-%d-%Y"],
    };
    let mut dates = formats
        .iter()
        .filter_map(|fmt| chrono::NaiveDate::parse_from_str(text.trim(), fmt).ok())
        .collect::<Vec<_>>();
    dates.sort();
    dates.dedup();
    if dates.len() == 1 {
        Some(dates[0].format("%Y-%m-%d").to_string())
    } else {
        None
    }
}
pub fn cell_address(row: usize, column: usize) -> String {
    let mut n = column;
    let mut prefix = String::new();
    while n > 0 {
        n -= 1;
        prefix.insert(0, (b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    format!("{prefix}{row}")
}
pub fn parse_address(address: &str) -> Result<(usize, usize)> {
    let address = address.trim().to_uppercase();
    let split = address
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| AppError::new("CELL", "单元格坐标无效"))?;
    let (letters, number) = address.split_at(split);
    if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(AppError::new("CELL", "单元格坐标无效"));
    }
    let col = letters
        .bytes()
        .try_fold(0usize, |n, c| {
            n.checked_mul(26)?.checked_add((c - b'A' + 1) as usize)
        })
        .ok_or_else(|| AppError::new("CELL", "单元格列超出范围"))?;
    let row = number
        .parse::<usize>()
        .map_err(|_| AppError::new("CELL", "单元格行无效"))?;
    if row == 0 || row > 1_048_576 || col > 16_384 {
        return Err(AppError::new("CELL", "单元格坐标超出 Excel 范围"));
    }
    Ok((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_header_skips_form_metadata_banners() {
        // 模拟类别 04：前两行是表单信息（含填写人:、审批领导:等冒号标签），第三行是实际表格列名
        let row1 = vec![
            Cell::Text("填写人:".into()),
            Cell::Text("张建国".into()),
            Cell::Text("所属机构:".into()),
            Cell::Text("北京朝阳分公司".into()),
            Cell::Text("申报日期:".into()),
            Cell::Text("2024-10-22".into()),
            Cell::Text("预算单号:".into()),
            Cell::Text("YS-2024-BJ01".into()),
        ];
        let row2 = vec![
            Cell::Text("审批领导:".into()),
            Cell::Text("王立群".into()),
            Cell::Text("费用归属部门:".into()),
            Cell::Text("市场部".into()),
            Cell::Text("申报月份:".into()),
            Cell::Text("2024-10".into()),
            Cell::Text("汇总结算方式:".into()),
            Cell::Text("网银转账".into()),
        ];
        let row3 = vec![
            Cell::Text("报销单号".into()),
            Cell::Text("费用发生日期".into()),
            Cell::Text("费用科目".into()),
            Cell::Text("费用详细事由".into()),
            Cell::Text("发票张数".into()),
            Cell::Text("申请报销金额".into()),
            Cell::Text("实批报销金额".into()),
            Cell::Text("结算支付方式".into()),
        ];
        let row4 = vec![
            Cell::Text("BX001".into()),
            Cell::Text("2024-10-02".into()),
            Cell::Text("差旅费".into()),
            Cell::Text("拜访客户".into()),
            Cell::Number(3.0),
            Cell::Number(1200.0),
            Cell::Number(1200.0),
            Cell::Text("银行转账".into()),
        ];
        let rows = vec![row1, row2, row3, row4];
        assert_eq!(suggest_header(&rows), 3);
    }

    #[test]
    fn test_suggest_column_type_semantic_guards() {
        let dates = vec![
            Cell::Text("2024-10-01".into()),
            Cell::Text("2024-10-02".into()),
            Cell::Text("2024-10-03".into()),
        ];
        let numbers = vec![
            Cell::Number(100.0),
            Cell::Number(200.0),
            Cell::Number(300.0),
        ];

        // 冒号标签强制为文本
        assert_eq!(suggest_column_type("申报日期:", &numbers), ColumnType::Text);
        assert_eq!(suggest_column_type("填写人：", &dates), ColumnType::Text);

        // 日期列名绝不能被建议为数字
        assert_eq!(suggest_column_type("申报日期", &numbers), ColumnType::Text);
        assert_eq!(suggest_column_type("费用发生日期", &numbers), ColumnType::Text);

        // 金额列名绝不能被建议为日期
        assert_eq!(suggest_column_type("申请报销金额", &dates), ColumnType::Text);

        // 正常金额列为数字
        assert_eq!(suggest_column_type("申请报销金额", &numbers), ColumnType::Number);

        // 正常日期列为日期
        assert_eq!(suggest_column_type("费用发生日期", &dates), ColumnType::Date);
    }
}
