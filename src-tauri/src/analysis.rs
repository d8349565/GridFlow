use crate::{
    engine::{cell_from_row, Engine},
    model::*,
};
use duckdb::{params_from_iter, types::Value, Connection};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn index(ds: &Dataset, key: &str) -> Result<usize> {
    ds.columns
        .iter()
        .position(|c| crate::reader::same_key(&c.key, key))
        .ok_or_else(|| AppError::new("FIELD", format!("字段不存在：{key}")))
}
pub(crate) fn conditions(ds: &Dataset, query: &DatasetQuery) -> Result<(String, Vec<Value>)> {
    let mut conditions = Vec::new();
    let mut args = Vec::new();
    for filter in &query.filters {
        let i = index(ds, &filter.key)?;
        let col = format!("c{i}");
        match filter.operator.as_str() {
            "in" => {
                if filter.values.is_empty() {
                    continue;
                }
                if filter.values.len() > 1000 {
                    return Err(AppError::new("FILTER", "一次筛选最多 1000 个值"));
                }
                conditions.push(format!(
                    "COALESCE({col},'') IN ({})",
                    vec!["?"; filter.values.len()].join(",")
                ));
                args.extend(filter.values.iter().cloned().map(Value::Text));
            }
            "empty" => conditions.push(format!("({col} IS NULL OR {col}='')")),
            "notEmpty" => conditions.push(format!("({col} IS NOT NULL AND {col}<>'')")),
            "contains" => {
                conditions.push(format!("LOWER(COALESCE({col},'')) LIKE ? ESCAPE '$'"));
                args.push(Value::Text(format!(
                    "%{}%",
                    escape_like(filter.values.first().map(String::as_str).unwrap_or(""))
                        .to_lowercase()
                )));
            }
            "eq" | "ne" | "gt" | "gte" | "lt" | "lte" => {
                let value = filter
                    .values
                    .first()
                    .ok_or_else(|| AppError::new("FILTER", "筛选缺少比较值"))?;
                let op = match filter.operator.as_str() {
                    "eq" => "=",
                    "ne" => "<>",
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                let numeric = ds.columns[i].data_type == ColumnType::Number
                    || ds.columns[i].suggested_type == ColumnType::Number;
                if numeric {
                    let value = crate::reader::parse_number(value)
                        .ok_or_else(|| AppError::new("FILTER", "比较值不是有效数值"))?;
                    conditions.push(format!("n{i} {op} ?"));
                    args.push(Value::Double(value));
                } else {
                    conditions.push(format!("{col} {op} ?"));
                    args.push(Value::Text(value.clone()));
                }
            }
            _ => return Err(AppError::new("FILTER", "不支持的筛选条件")),
        }
    }
    if !query.search.trim().is_empty() {
        let search = format!("%{}%", escape_like(query.search.trim()).to_lowercase());
        conditions.push(format!(
            "({})",
            (0..ds.columns.len())
                .map(|i| format!("LOWER(COALESCE(c{i},'')) LIKE ? ESCAPE '$'"))
                .collect::<Vec<_>>()
                .join(" OR ")
        ));
        args.extend((0..ds.columns.len()).map(|_| Value::Text(search.clone())));
    }
    Ok((
        if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        },
        args,
    ))
}
fn escape_like(s: &str) -> String {
    s.replace('$', "$$").replace('%', "$%").replace('_', "$_")
}
pub(crate) fn visible(ds: &Dataset) -> Vec<usize> {
    ds.columns
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            !ds.rules
                .columns
                .iter()
                .any(|r| crate::reader::same_key(&r.key, &c.key) && r.hidden)
        })
        .map(|(i, _)| i)
        .collect()
}
pub(crate) fn order(ds: &Dataset, query: &DatasetQuery) -> Result<String> {
    let mut orders = Vec::new();
    for sort in &query.sorts {
        let i = index(ds, &sort.key)?;
        let col = &ds.columns[i];
        let expr =
            if col.data_type == ColumnType::Number || col.suggested_type == ColumnType::Number {
                format!("n{i}")
            } else {
                format!("c{i}")
            };
        orders.push(format!(
            "{expr} {} NULLS LAST",
            if sort.descending { "DESC" } else { "ASC" }
        ));
    }
    orders.push("_row ASC".into());
    Ok(format!(" ORDER BY {}", orders.join(",")))
}
impl Engine {
    pub fn query(&self, query: DatasetQuery) -> Result<DataPage> {
        let ds = self.dataset(&query.dataset_id)?;
        let conn = self.conn()?;
        let indices = visible(&ds);
        let (condition, args) = conditions(&ds, &query)?;
        let total = conn.query_row(
            &format!("SELECT COUNT(*) FROM d_{}{condition}", ds.id),
            params_from_iter(args.clone()),
            |r| r.get::<_, i64>(0),
        )? as usize;
        let selection = indices
            .iter()
            .flat_map(|i| [format!("c{i}"), format!("t{i}")])
            .collect::<Vec<_>>()
            .join(",");
        if selection.is_empty() {
            return Err(AppError::new("COLUMN", "所有列都被隐藏，请显示至少一列"));
        }
        let sql = format!(
            "SELECT _row,{selection} FROM d_{}{condition}{} LIMIT ? OFFSET ?",
            ds.id,
            order(&ds, &query)?
        );
        let mut args = args;
        args.extend([
            Value::BigInt(query.limit.clamp(1, 1000) as i64),
            Value::BigInt(query.offset as i64),
        ]);
        let mut stmt = conn.prepare(&sql)?;
        let mut cursor = stmt.query(params_from_iter(args))?;
        let mut rows = Vec::new();
        let mut numbers = Vec::new();
        while let Some(row) = cursor.next()? {
            numbers.push(row.get::<_, i64>(0)? as usize);
            rows.push(
                (0..indices.len())
                    .map(|i| cell_from_row(row, 1 + 2 * i))
                    .collect::<Result<_>>()?,
            );
        }
        Ok(DataPage {
            columns: indices.iter().map(|i| ds.columns[*i].clone()).collect(),
            rows,
            row_numbers: numbers,
            total,
            offset: query.offset,
            sample: ds.sample,
        })
    }
    pub fn distinct_values(
        &self,
        query: DatasetQuery,
        key: String,
        offset: usize,
    ) -> Result<Vec<String>> {
        let ds = self.dataset(&query.dataset_id)?;
        let i = index(&ds, &key)?;
        let conn = self.conn()?;
        let (condition, mut args) = conditions(&ds, &query)?;
        args.push(Value::BigInt(offset as i64));
        let mut stmt=conn.prepare(&format!("SELECT DISTINCT COALESCE(c{i},'') AS value FROM d_{}{condition} ORDER BY value LIMIT 200 OFFSET ?",ds.id))?;
        Ok(stmt
            .query_map(params_from_iter(args), |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn aggregate(&self, spec: PivotSpec, limit: Option<usize>) -> Result<Analysis> {
        self.guard()?;
        let _gate = self.heavy.lock().unwrap();
        let ds = self.dataset(&spec.query.dataset_id)?;
        let conn = self.conn()?;
        aggregate_query(&conn, &ds, spec, limit)
    }
}
#[derive(Clone, Default)]
struct Stats {
    sum: Option<f64>,
    count: i64,
    min: Option<f64>,
    max: Option<f64>,
    non_empty: i64,
}
impl Stats {
    fn add(&mut self, b: &Self) {
        if let Some(sum) = b.sum {
            self.sum = Some(self.sum.unwrap_or(0.) + sum);
        }
        self.count += b.count;
        self.non_empty += b.non_empty;
        if let Some(x) = b.min {
            self.min = Some(self.min.map_or(x, |v| v.min(x)));
        }
        if let Some(x) = b.max {
            self.max = Some(self.max.map_or(x, |v| v.max(x)));
        }
    }
    fn value(&self, aggregation: &str) -> Cell {
        let value = match aggregation {
            "sum" => self.sum,
            "count" => Some(self.non_empty as f64),
            "avg" => {
                if self.count == 0 {
                    None
                } else {
                    self.sum.map(|s| s / self.count as f64)
                }
            }
            "min" => self.min,
            "max" => self.max,
            _ => None,
        };
        value.map(Cell::Number).unwrap_or(Cell::Empty)
    }
}
pub(crate) fn aggregate_query(
    conn: &Connection,
    ds: &Dataset,
    spec: PivotSpec,
    limit: Option<usize>,
) -> Result<Analysis> {
    if spec.row_fields.is_empty() && spec.column_fields.is_empty() {
        return Err(AppError::new("PIVOT", "请选择至少一个行或列字段"));
    }
    if spec.value_fields.is_empty() {
        return Err(AppError::new("PIVOT", "请选择至少一个数值字段"));
    }
    if !["sum", "count", "avg", "min", "max"].contains(&spec.aggregation.as_str()) {
        return Err(AppError::new("PIVOT", "不支持的汇总方式"));
    }
    let dims = spec
        .row_fields
        .iter()
        .chain(&spec.column_fields)
        .map(|k| index(ds, k))
        .collect::<Result<Vec<_>>>()?;
    let values = spec
        .value_fields
        .iter()
        .map(|k| index(ds, k))
        .collect::<Result<Vec<_>>>()?;
    let mut select = dims.iter().map(|i| format!("c{i}")).collect::<Vec<_>>();
    for i in &values {
        let numeric = format!("n{i}");
        select.extend([
            format!("SUM({numeric})"),
            format!("COUNT({numeric})"),
            format!("MIN({numeric})"),
            format!("MAX({numeric})"),
            format!("COUNT(NULLIF(c{i},''))"),
        ]);
    }
    let (condition, args) = conditions(ds, &spec.query)?;
    let total_records = conn.query_row(
        &format!("SELECT COUNT(*) FROM d_{}{condition}", ds.id),
        params_from_iter(args.clone()),
        |r| r.get::<_, i64>(0),
    )? as usize;
    let sql = format!(
        "SELECT {} FROM d_{}{condition} GROUP BY {}",
        select.join(","),
        ds.id,
        dims.iter()
            .map(|i| format!("c{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    type Groups = BTreeMap<Vec<Option<String>>, BTreeMap<Vec<Option<String>>, Vec<Stats>>>;
    let mut groups = Groups::new();
    let mut column_groups = BTreeSet::new();
    let mut stmt = conn.prepare(&sql)?;
    let mut cursor = stmt.query(params_from_iter(args))?;
    while let Some(row) = cursor.next()? {
        let dimensions = (0..dims.len())
            .map(|i| row.get::<_, Option<String>>(i))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let row_key = dimensions[..spec.row_fields.len()].to_vec();
        let column_key = dimensions[spec.row_fields.len()..].to_vec();
        column_groups.insert(column_key.clone());
        let stats = (0..values.len())
            .map(|j| {
                let i = dims.len() + j * 5;
                Ok::<_, duckdb::Error>(Stats {
                    sum: row.get(i)?,
                    count: row.get(i + 1)?,
                    min: row.get(i + 2)?,
                    max: row.get(i + 3)?,
                    non_empty: row.get(i + 4)?,
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        groups.entry(row_key).or_default().insert(column_key, stats);
    }
    if column_groups.len() * values.len() > 1024 {
        return Err(AppError::new(
            "PIVOT_WIDTH",
            "透视结果超过 1024 列，请减少列维度或筛选值",
        ));
    }
    if groups
        .len()
        .saturating_mul(column_groups.len() * values.len() + spec.row_fields.len() + values.len())
        > 2_000_000
    {
        return Err(AppError::new(
            "PIVOT_SIZE",
            "透视行列组合超过 200 万个单元格，请筛选数据或减少行列维度后再分析",
        ));
    }
    let mut columns = spec
        .row_fields
        .iter()
        .map(|k| ds.columns[index(ds, k).unwrap()].clone())
        .collect::<Vec<_>>();
    for key in &column_groups {
        for (j, i) in values.iter().enumerate() {
            let label = key
                .iter()
                .map(|s| s.clone().unwrap_or_else(|| "(空白)".into()))
                .collect::<Vec<_>>()
                .join(" / ");
            columns.push(Column {
                key: format!("value:{}:{j}", columns.len()),
                name: if label.is_empty() {
                    ds.columns[*i].name.clone()
                } else {
                    format!("{label} · {}", ds.columns[*i].name)
                },
                data_type: ColumnType::Number,
                suggested_type: ColumnType::Number,
            });
        }
    }
    if spec.show_row_totals && !spec.column_fields.is_empty() {
        for (j, i) in values.iter().enumerate() {
            columns.push(Column {
                key: format!("total:{j}"),
                name: format!("合计 · {}", ds.columns[*i].name),
                data_type: ColumnType::Number,
                suggested_type: ColumnType::Number,
            });
        }
    }
    let mut result = Vec::new();
    let mut grand_by_column = BTreeMap::<Vec<Option<String>>, Vec<Stats>>::new();
    let mut grand = vec![Stats::default(); values.len()];
    for (row_key, parts) in &groups {
        let mut cells = row_key
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let column = &ds.columns[index(ds, &spec.row_fields[i]).unwrap()];
                if column.data_type == ColumnType::Number
                    || (column.data_type == ColumnType::Auto
                        && column.suggested_type == ColumnType::Number)
                {
                    v.as_deref()
                        .and_then(crate::reader::parse_number)
                        .map(Cell::Number)
                        .unwrap_or(Cell::Empty)
                } else {
                    v.clone().map(Cell::Text).unwrap_or(Cell::Empty)
                }
            })
            .collect::<Vec<_>>();
        let mut totals = vec![Stats::default(); values.len()];
        for key in &column_groups {
            let stats = parts
                .get(key)
                .cloned()
                .unwrap_or_else(|| vec![Stats::default(); values.len()]);
            let grand_column = grand_by_column
                .entry(key.clone())
                .or_insert_with(|| vec![Stats::default(); values.len()]);
            for j in 0..values.len() {
                cells.push(stats[j].value(&spec.aggregation));
                totals[j].add(&stats[j]);
                grand_column[j].add(&stats[j]);
                grand[j].add(&stats[j]);
            }
        }
        if spec.show_row_totals && !spec.column_fields.is_empty() {
            cells.extend(totals.iter().map(|s| s.value(&spec.aggregation)));
        }
        result.push(cells);
    }
    if !spec.custom_order.is_empty() && !spec.row_fields.is_empty() {
        result.sort_by_key(|r| {
            spec.custom_order
                .iter()
                .position(|s| *s == r[0].text())
                .unwrap_or(usize::MAX)
        });
    }
    if let Some(sort) = &spec.sort {
        let i = columns
            .iter()
            .position(|c| c.key == sort.key)
            .ok_or_else(|| AppError::new("SORT", "排序字段不存在"))?;
        result.sort_by(|a, b| {
            let cmp = match (&a[i], &b[i]) {
                (Cell::Number(a), Cell::Number(b)) => a.total_cmp(b),
                _ => a[i].text().cmp(&b[i].text()),
            };
            if sort.descending {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }
    let group_count = result.len();
    let truncated = limit.is_some_and(|n| result.len() > n);
    if let Some(limit) = limit {
        result.truncate(limit);
    }
    if spec.show_row_totals && !groups.is_empty() {
        let mut total = vec![Cell::Text("总计".into())];
        total.resize(spec.row_fields.len(), Cell::Empty);
        if spec.row_fields.is_empty() {
            total.clear();
        }
        for key in &column_groups {
            total.extend(
                grand_by_column[key]
                    .iter()
                    .map(|s| s.value(&spec.aggregation)),
            );
        }
        if !spec.column_fields.is_empty() {
            total.extend(grand.iter().map(|s| s.value(&spec.aggregation)));
        }
        result.push(total);
    }
    Ok(Analysis {
        columns,
        rows: result,
        total_records,
        group_count,
        truncated,
        spec,
    })
}
