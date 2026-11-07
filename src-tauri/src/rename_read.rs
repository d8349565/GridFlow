//! Bounded, sparse reads for renaming. Worksheet dimensions never decide whether a rule can run.
use crate::{appearance, model::*};
use calamine::{Data, DataRef, Reader, Sheets};
use quick_xml::events::Event;
use std::{collections::{BTreeMap, HashMap, HashSet}, fs::File, io::BufReader, path::Path};

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Scope { pub areas: Vec<SheetRange>, pub filled_rows: Vec<(usize, usize)> }
impl Scope {
    pub fn contains(&self, row: usize, column: usize) -> bool { self.areas.iter().any(|r| row >= r.start_row && row <= r.end_row && column >= r.start_column && column <= r.end_column) }
    fn count_row(&self, row: usize) -> bool { self.filled_rows.iter().any(|(start,end)| row >= *start && row <= *end) }
    fn required_coordinates(&self) -> Option<HashSet<(usize,usize)>> {
        if !self.filled_rows.is_empty() || self.areas.iter().map(|r|(r.end_row-r.start_row+1).saturating_mul(r.end_column-r.start_column+1)).sum::<usize>()>4096 {return None;}
        Some(self.areas.iter().flat_map(|r|(r.start_row..=r.end_row).flat_map(move |row|(r.start_column..=r.end_column).map(move |column|(row,column)))).collect())
    }
}
pub(crate) struct ReadCell { pub value: Data, pub formula: bool }
#[derive(Default)]
pub(crate) struct SheetCells {
    pub cells: BTreeMap<(usize, usize), ReadCell>,
    pub filled: HashMap<usize, usize>,
    pub rows: usize,
    pub columns: usize,
}
impl SheetCells {
    fn insert(&mut self, position: (usize, usize), value: Data, max_cells: usize) -> Result<()> {
        if self.cells.len() >= max_cells && !self.cells.contains_key(&position) { return Err(AppError::new("LIMIT", "规则需要保留的单元格超过安全上限，请缩小校验区域")); }
        self.cells.entry(position).or_insert(ReadCell { value: Data::Empty, formula: false }).value = value;
        Ok(())
    }
}
fn formula_flags(path: &Path, sheet: &str, scope: &Scope, out: &mut SheetCells, max_cells: usize) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|e| AppError::new("EXCEL",e.to_string()))?;
    let workbook = appearance::xml(&mut zip,"xl/workbook.xml")?;
    let part = appearance::sheet_path(&mut zip,&workbook,sheet)?;
    let file = zip.by_name(&part).map_err(|e| AppError::new("SHEET",e.to_string()))?;
    let mut xml = quick_xml::Reader::from_reader(BufReader::new(file));
    let mut buffer = Vec::new(); let mut current = None; let mut row = 1; let mut column = 1;
    let mut remaining=scope.required_coordinates();
    loop {
        let event=xml.read_event_into(&mut buffer).map_err(|e| AppError::new("EXCEL",e.to_string()))?;
        let empty=matches!(&event,Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"row" => { if let Some(value)=e.attributes().flatten().find(|a| a.key.as_ref()==b"r") { row=String::from_utf8_lossy(&value.value).parse().unwrap_or(row); } column=1;if empty {row+=1;} },
                b"c" => {
                    let position=e.attributes().flatten().find(|a| a.key.as_ref()==b"r").and_then(|a| crate::reader::parse_address(&String::from_utf8_lossy(&a.value)).ok()).unwrap_or((row,column));
                    row=position.0; column=position.1+1; current=(!empty&&scope.contains(position.0,position.1)).then_some(position);
                    if empty {if let Some(pending)=remaining.as_mut() {pending.remove(&position);}}
                },
                b"f" => if let Some(position)=current { if !out.cells.contains_key(&position) { out.insert(position,Data::Empty,max_cells)?; } out.cells.get_mut(&position).unwrap().formula=true; },
                _ => {},
            },
            Event::End(e) if e.local_name().as_ref()==b"c" => {if let (Some(position),Some(pending))=(current,remaining.as_mut()) {pending.remove(&position);}current=None;},
            Event::End(e) if e.local_name().as_ref()==b"row" => {row+=1;column=1;},
            Event::Eof => break,
            _ => {},
        }
        if remaining.as_ref().is_some_and(|s|s.is_empty()) {break;}
        buffer.clear();
    }
    Ok(())
}
pub(crate) fn read(book: &mut Sheets<BufReader<File>>, path: &Path, sheet: &str, scope: &Scope, max_cells: usize) -> Result<SheetCells> {
    let mut out=SheetCells::default();
    if let Sheets::Xlsx(xlsx)=book {
        let mut cells=xlsx.worksheet_cells_reader(sheet).map_err(|e| AppError::new("SHEET",format!("无法读取工作表“{sheet}”：{e}")))?;
        let mut remaining=scope.required_coordinates();
        let end=cells.dimensions().end; out.rows=(end.0 as usize+1).min(1_048_576); out.columns=(end.1 as usize+1).min(16384);
        while let Some(cell)=cells.next_cell().map_err(|e| AppError::new("EXCEL",e.to_string()))? {
            let pos=cell.get_position(); let (row,column)=(pos.0 as usize+1,pos.1 as usize+1);
            out.rows=out.rows.max(row); out.columns=out.columns.max(column);
            let nonempty=match cell.get_value() {DataRef::Empty=>false,DataRef::String(v)=>!v.trim().is_empty(),DataRef::SharedString(v)=>!v.trim().is_empty(),_=>true};
            if nonempty && scope.count_row(row) { *out.filled.entry(row).or_default()+=1; }
            if scope.contains(row,column) && !matches!(cell.get_value(),DataRef::Empty) { out.insert((row,column),Data::from(cell.get_value().clone()),max_cells)?; }
            if let Some(pending)=remaining.as_mut() {pending.remove(&(row,column));if pending.is_empty() {break;}}
        }
        drop(cells);
        formula_flags(path,sheet,scope,&mut out,max_cells)?;
    } else {
        let values=book.worksheet_range(sheet).map_err(|e| AppError::new("SHEET",e.to_string()))?;
        let formulas=book.worksheet_formula(sheet).map_err(|e| AppError::new("SHEET",e.to_string()))?;
        for end in [values.end(),formulas.end()].into_iter().flatten() {out.rows=out.rows.max(end.0 as usize+1);out.columns=out.columns.max(end.1 as usize+1);}
        let start=values.start().unwrap_or((0,0));
        for (r,c,value) in values.used_cells() {
            let (row,column)=(start.0 as usize+r+1,start.1 as usize+c+1);
            if !matches!(value,Data::String(v) if v.trim().is_empty()) && scope.count_row(row) {*out.filled.entry(row).or_default()+=1;}
            if scope.contains(row,column) {out.insert((row,column),value.clone(),max_cells)?;}
        }
        let start=formulas.start().unwrap_or((0,0));
        for (r,c,_) in formulas.used_cells() {let pos=(start.0 as usize+r+1,start.1 as usize+c+1);if scope.contains(pos.0,pos.1) {if !out.cells.contains_key(&pos) {out.insert(pos,Data::Empty,max_cells)?;}out.cells.get_mut(&pos).unwrap().formula=true;}}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_cell_stays_sparse_when_unrelated_data_reaches_the_last_excel_cell() {
        let temp=tempfile::TempDir::new_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();let path=temp.path().join("按规则取值.xlsx");
        let mut book=rust_xlsxwriter::Workbook::new();let sheet=book.add_worksheet();sheet.write_string(0,0,"001").unwrap();sheet.write_string(0,100,"备注").unwrap();sheet.write_number(1_048_575,16383,0.).unwrap();book.save(&path).unwrap();
        let mut book=calamine::open_workbook_auto(&path).unwrap();let scope=Scope {areas:vec![SheetRange {start_row:1,end_row:1,start_column:1,end_column:1}],filled_rows:vec![(1,1)]};
        let data=read(&mut book,&path,"Sheet1",&scope,1).unwrap();assert_eq!(data.cells.len(),1);assert_eq!(data.cells[&(1,1)].value,Data::String("001".into()));assert_eq!(data.filled.get(&1),Some(&2));assert_eq!((data.rows,data.columns),(1_048_576,16384));
    }
}
