use crate::model::*;
use calamine::{open_workbook_auto, Data, Reader};
use std::{collections::HashMap, path::Path};

pub struct ReadSheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
    pub width: usize,
}
pub struct ReadFile {
    pub sheets: Vec<ReadSheet>,
    pub encoding: Option<String>,
}

pub fn read(path: &Path) -> Result<ReadFile> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match extension.as_str() {
        "xlsx" | "xls" => read_excel(path),
        "csv" => read_csv(path),
        _ => Err(AppError::new(
            "UNSUPPORTED",
            "请选择 .xlsx、.xls 或 .csv 文件",
        )),
    }
}
fn read_excel(path: &Path) -> Result<ReadFile> {
    let mut book = open_workbook_auto(path)
        .map_err(|e| AppError::new("EXCEL", format!("Excel 无法读取（可能已损坏或加密）：{e}")))?;
    let mut sheets = Vec::new();
    for name in book.sheet_names().to_owned() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| AppError::new("EXCEL", e.to_string()))?;
        let formulas = book.worksheet_formula(&name).ok();
        let ends = [range.end(), formulas.as_ref().and_then(|f| f.end())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let height = ends.iter().map(|p| p.0 as usize + 1).max().unwrap_or(0);
        let width = ends.iter().map(|p| p.1 as usize + 1).max().unwrap_or(0);
        if width > 2048 || height.saturating_mul(width) > 20_000_000 {
            return Err(AppError::new(
                "LIMIT",
                "工作表展开范围超过 2048 列或 2000 万个单元格，请缩小源表范围",
            ));
        }
        let mut rows = Vec::with_capacity(height);
        for r in 0..height {
            let mut cells = Vec::with_capacity(width);
            for c in 0..width {
                let value = range
                    .get_value((r as u32, c as u32))
                    .unwrap_or(&Data::Empty);
                let cell = match value {
                    Data::Empty => {
                        let has_formula = formulas
                            .as_ref()
                            .and_then(|f| f.get_value((r as u32, c as u32)))
                            .is_some_and(|v| !v.is_empty());
                        if has_formula {
                            Cell::Text("#公式无缓存".into())
                        } else {
                            Cell::Empty
                        }
                    }
                    Data::String(v) => Cell::Text(v.clone()),
                    Data::Int(v) => Cell::Number(*v as f64),
                    Data::Float(v) => Cell::Number(*v),
                    Data::Bool(v) => Cell::Bool(*v),
                    Data::DateTime(v) => v
                        .as_datetime()
                        .map(|d| Cell::Date(d.format("%Y-%m-%d %H:%M:%S").to_string()))
                        .unwrap_or(Cell::Number(v.as_f64())),
                    Data::DateTimeIso(v) => Cell::Date(v.clone()),
                    Data::DurationIso(v) => Cell::Text(v.clone()),
                    Data::Error(v) => Cell::Text(format!("#Excel错误:{v:?}")),
                };
                cells.push(cell);
            }
            rows.push(cells);
        }

        sheets.push(ReadSheet { name, rows, width });
    }
    if sheets.is_empty() {
        return Err(AppError::new("EMPTY", "工作簿没有工作表"));
    }
    Ok(ReadFile {
        sheets,
        encoding: None,
    })
}
fn read_csv(path: &Path) -> Result<ReadFile> {
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
    if key == "__source" || key.starts_with("fixed:") || key.starts_with("input:") {
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
    if values
        .iter()
        .filter(|v| matches!(v, Cell::Date(_)) || parse_date(&v.text()).is_some())
        .count()
        * 5
        >= values.len() * 4
    {
        return ColumnType::Date;
    }
    if values
        .iter()
        .filter(|v| parse_number(&v.text()).is_some())
        .count()
        * 5
        >= values.len() * 4
    {
        return ColumnType::Number;
    }
    ColumnType::Text
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
