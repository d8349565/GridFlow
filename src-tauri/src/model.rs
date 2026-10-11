use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Cell {
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
    Date(String),
}
impl Cell {
    pub fn text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(v) | Self::Date(v) => v.clone(),
            Self::Number(v) => v.to_string(),
            Self::Bool(v) => v.to_string(),
        }
    }
    pub fn kind(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Text(_) => 1,
            Self::Number(_) => 2,
            Self::Bool(_) => 3,
            Self::Date(_) => 4,
        }
    }
    pub fn from_parts(text: Option<String>, kind: u8) -> Self {
        let s = text.unwrap_or_default();
        match kind {
            0 => Self::Empty,
            2 => s.parse().map(Self::Number).unwrap_or(Self::Text(s)),
            3 => Self::Bool(s == "true"),
            4 => Self::Date(s),
            _ => Self::Text(s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColumnType {
    #[default]
    Auto,
    Text,
    Number,
    Date,
    Skip,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub key: String,
    pub name: String,
    pub data_type: ColumnType,
    pub suggested_type: ColumnType,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSheet {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub rows: usize,
    pub width: usize,
    pub suggested_header_row: usize,
    pub columns: Vec<Column>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub imported_at: String,
    pub stored_path: String,
    pub encoding: Option<String>,
    pub sheets: Vec<SourceSheet>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelection {
    pub sheet_id: String,
    pub enabled: bool,
    pub header_row: usize,
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub header_range: Option<SheetRange>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SheetRange {
    pub start_row: usize,
    pub end_row: usize,
    pub start_column: usize,
    pub end_column: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
pub struct CellStyle {
    pub bold: bool,
    pub italic: bool,
    pub font_size: f64,
    pub font_name: String,
    pub underline: bool,
    pub strike: bool,
    pub color: String,
    pub background: String,
    pub align: String,
    pub vertical_align: String,
    pub rotation: i32,
    pub indent: usize,
    pub wrap: bool,
    pub number_format: String,
    pub borders: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CellAppearance {
    pub row: usize,
    pub column: usize,
    pub style: usize,
    pub raw_number: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
pub struct SheetAppearance {
    pub widths: Vec<f64>,
    pub column_widths: std::collections::HashMap<usize, f64>,
    pub styles: Vec<CellStyle>,
    pub cells: Vec<CellAppearance>,
    pub merges: Vec<SheetRange>,
    pub row_heights: std::collections::HashMap<usize, f64>,
    pub default_row_height: f64,
    pub date_1904: bool,
    pub show_grid_lines: bool,
    pub note: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRule {
    pub key: String,
    pub name: String,
    pub data_type: ColumnType,
    pub hidden: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub source_key: String,
    pub target_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixedCellRule {
    pub id: String,
    pub name: String,
    pub sheet_name: String,
    pub address: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleSet {
    pub version: u32,
    pub sources: Vec<SourceSelection>,
    pub columns: Vec<ColumnRule>,
    pub mappings: Vec<ColumnMapping>,
    pub fixed_cells: Vec<FixedCellRule>,
    pub merge_strategy: String,
    pub remove_empty_rows: bool,
    pub remove_duplicates: bool,
    pub duplicate_columns: Vec<String>,
    pub keep_strategy: String,
    pub empty_columns: Vec<String>,
    pub clean_numeric: bool,
    pub parse_dates: bool,
    pub include_source: bool,
    pub date_formats: std::collections::HashMap<String, String>,
}
impl Default for RuleSet {
    fn default() -> Self {
        Self {
            version: 2,
            sources: vec![],
            columns: vec![],
            mappings: vec![],
            fixed_cells: vec![],
            merge_strategy: "outer".into(),
            remove_empty_rows: false,
            remove_duplicates: false,
            duplicate_columns: vec![],
            keep_strategy: "first".into(),
            clean_numeric: false,
            parse_dates: false,
            include_source: true,
            date_formats: Default::default(),
            empty_columns: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub dataset_id: String,
    pub file_id: String,
    pub sheet_id: String,
    pub row: usize,
    pub column: String,
    pub original_value: String,
    pub message: String,
    pub source_column: usize,
    pub file_name: String,
    pub sheet_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversionExample {
    pub before: Cell,
    pub after: Cell,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TypeSuggestion {
    pub key: String,
    pub name: String,
    pub suggested_type: ColumnType,
    pub examples: Vec<ConversionExample>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub action: String,
    pub subject: String,
    pub timestamp: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub input_rows: usize,
    pub output_rows: usize,
    pub empty_removed: usize,
    pub duplicates_removed: usize,
    pub issue_count: usize,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub columns: Vec<Column>,
    pub summary: Summary,
    pub sample: bool,
    pub rules: RuleSet,
    pub exports: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Pending,
    Running,
    Cancelling,
    Cancelled,
    Succeeded,
    Failed,
    Interrupted,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub state: TaskState,
    pub phase: String,
    pub progress: u8,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub dataset_id: Option<String>,
    pub rules: RuleSet,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SheetPreset {
    pub name: String,
    pub header_row: usize,
    pub enabled: bool,
    #[serde(default)]
    pub workbook_sheets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub header_range: Option<SheetRange>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Scheme {
    pub id: String,
    pub name: String,
    pub rules: RuleSet,
    pub sheets: Vec<SheetPreset>,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub password_hash: Option<String>,
    pub last_export_directory: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Workspace {
    pub files: Vec<SourceFile>,
    pub datasets: Vec<Dataset>,
    pub tasks: Vec<Task>,
    pub schemes: Vec<Scheme>,
    pub rules: RuleSet,
    pub active_sheet_id: Option<String>,
    pub settings: Settings,
    pub data_directory: String,
    pub locked: bool,
    pub activities: Vec<Activity>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub path: String,
    pub file: Option<SourceFile>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DatasetQuery {
    pub dataset_id: String,
    pub offset: usize,
    pub limit: usize,
    pub sorts: Vec<Sort>,
    pub filters: Vec<Filter>,
    pub search: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Sort {
    pub key: String,
    pub descending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub key: String,
    pub operator: String,
    pub values: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataPage {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub row_numbers: Vec<usize>,
    pub total: usize,
    pub offset: usize,
    pub sample: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PivotSpec {
    pub query: DatasetQuery,
    pub row_fields: Vec<String>,
    pub column_fields: Vec<String>,
    pub value_fields: Vec<String>,
    pub aggregation: String,
    pub show_row_totals: bool,
    pub sort: Option<Sort>,
    pub custom_order: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub total_records: usize,
    pub group_count: usize,
    pub truncated: bool,
    pub spec: PivotSpec,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            field: None,
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AppError {}
impl From<duckdb::Error> for AppError {
    fn from(e: duckdb::Error) -> Self {
        Self::new("DATABASE", e.to_string())
    }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new("IO", e.to_string())
    }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("FORMAT", e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, AppError>;
pub fn now() -> String {
    chrono::Local::now().to_rfc3339()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn generate_types(path: &std::path::Path) -> std::io::Result<()> {
    use std::fmt::Write;
    let mut text = String::from("// Generated from Rust DTOs. Run npm run types; do not edit.\n");
    macro_rules! decl { ($($t:ty),*)=>{$(writeln!(text,"export {}",<$t as TS>::decl()).unwrap();)*}; }
    decl!(
        Cell,
        ColumnType,
        Column,
        SourceSheet,
        SourceFile,
        SourceSelection,
        SheetRange,
        CellStyle,
        CellAppearance,
        SheetAppearance,
        ColumnRule,
        ColumnMapping,
        FixedCellRule,
        RuleSet,
        Issue,
        ConversionExample,
        TypeSuggestion,
        Activity,
        Summary,
        Dataset,
        TaskState,
        Task,
        SheetPreset,
        Scheme,
        Settings,
        Workspace,
        ImportItem,
        DatasetQuery,
        Sort,
        Filter,
        DataPage,
        PivotSpec,
        Analysis,
        AppError
    );
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, text)
}
