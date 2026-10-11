//! Read presentation metadata only. Values remain in the paged workspace tables.
use crate::{model::*, reader};
use quick_xml::{events::Event, Reader};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

fn attrs(e: &quick_xml::events::BytesStart<'_>, decoder: quick_xml::encoding::Decoder) -> HashMap<String, String> {
    e.attributes().flatten().filter_map(|a| {
        Some((String::from_utf8_lossy(a.key.as_ref()).split(':').last()?.into(), a.decode_and_unescape_value(decoder).ok()?.into_owned()))
    }).collect()
}
fn xml(zip: &mut zip::ZipArchive<File>, name: &str) -> Result<String> {
    let file = zip.by_name(name).map_err(|e| AppError::new("LAYOUT", e.to_string()))?;
    if file.size() > 80_000_000 { return Err(AppError::new("LAYOUT", "格式文件较大，使用标准网格显示")); }
    let mut value = String::new();
    file.take(80_000_001).read_to_string(&mut value)?;
    Ok(value)
}
fn visit(value: &str, mut action: impl FnMut(&str, &HashMap<String, String>, bool)) -> Result<()> {
    let mut r = Reader::from_str(value);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => action(&String::from_utf8_lossy(e.local_name().as_ref()), &attrs(&e, r.decoder()), false),
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                action(&tag, &attrs(&e, r.decoder()), false); action(&tag, &HashMap::new(), true);
            },
            Ok(Event::End(e)) => action(&String::from_utf8_lossy(e.local_name().as_ref()), &HashMap::new(), true),
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::new("LAYOUT", format!("无法读取原表格式：{e}"))),
            _ => {}
        }
    }
    Ok(())
}
fn index(a: &HashMap<String, String>, key: &str) -> usize { a.get(key).and_then(|s| s.parse().ok()).unwrap_or(0) }
pub(crate) fn indexed_palette() -> Vec<String> {
    "000000 FFFFFF FF0000 00FF00 0000FF FFFF00 FF00FF 00FFFF 000000 FFFFFF FF0000 00FF00 0000FF FFFF00 FF00FF 00FFFF 800000 008000 000080 808000 800080 008080 C0C0C0 808080 9999FF 993366 FFFFCC CCFFFF 660066 FF8080 0066CC CCCCFF 000080 FF00FF FFFF00 00FFFF 800080 800000 008080 0000FF 00CCFF CCFFFF CCFFCC FFFF99 99CCFF FF99CC CC99FF FFCC99 3366FF 33CCCC 99CC00 FFCC00 FF9900 FF6600 666699 969696 003366 339966 003300 333300 993300 993366 333399 333333".split_whitespace().map(|v|format!("#{v}")).collect()
}
fn colour(a: &HashMap<String, String>, theme: &[String], indexed: &[String]) -> String {
    let rgb = a.get("rgb").map(|v| if v.len() == 8 { &v[2..] } else { v.as_str() });
    let c=if let Some(rgb) = rgb.filter(|v| v.len() == 6 && v.chars().all(|c| c.is_ascii_hexdigit())) { format!("#{rgb}") }
        else if a.contains_key("theme") { theme.get(index(a,"theme")).cloned().unwrap_or_default() }
        else if a.contains_key("indexed") { indexed.get(index(a,"indexed")).cloned().unwrap_or_default() } else { String::new() };
    if let Some(t)=a.get("tint").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()) { tint(&c,t.clamp(-1.,1.)) } else { c }
}
fn tint(c:&str,t:f64)->String {
    let Ok(rgb)=u32::from_str_radix(c.trim_start_matches('#'),16) else {return c.into()};
    let (r,g,b)=(((rgb>>16)&255) as f64/255.,((rgb>>8)&255) as f64/255.,(rgb&255) as f64/255.);
    let max=r.max(g).max(b);let min=r.min(g).min(b);let d=max-min;let l=(max+min)/2.;
    let s=if d==0. {0.} else if l>0.5 {d/(2.-max-min)} else {d/(max+min)};
    let h=if d==0. {0.}else if max==r {(g-b)/d+if g<b {6.}else{0.}}else if max==g {(b-r)/d+2.}else{(r-g)/d+4.};
    let l=if t<0. {l*(1.+t)}else{l*(1.-t)+t};let q=if l<0.5 {l*(1.+s)}else{l+s-l*s};let p=2.*l-q;
    let hue=|x:f64| {let x=x.rem_euclid(1.);if x<1./6. {p+(q-p)*6.*x}else if x<0.5 {q}else if x<2./3. {p+(q-p)*(2./3.-x)*6.}else{p}};
    let v=if s==0. {[l,l,l]}else{[hue(h/6.+1./3.),hue(h/6.),hue(h/6.-1./3.)]};
    format!("#{:02X}{:02X}{:02X}",(v[0]*255.).round() as u8,(v[1]*255.).round() as u8,(v[2]*255.).round() as u8)
}
fn theme(value:&str)->Vec<String> {
    let names=["lt1","dk1","lt2","dk2","accent1","accent2","accent3","accent4","accent5","accent6","hlink","folHlink"];
    let mut out=["#ffffff","#000000","#e7e6e6","#44546a","#4472c4","#ed7d31","#a5a5a5","#ffc000","#5b9bd5","#70ad47","#0000ff","#800080"].map(String::from).to_vec();let mut current=None;
    let _=visit(value,|tag,a,end| {if let Some(i)=names.iter().position(|v|*v==tag) { current=if end{None}else{Some(i)}; }
        if !end && ["srgbClr","sysClr"].contains(&tag) {if let Some(i)=current {if let Some(rgb)=a.get("val").filter(|v|v.len()==6).or(a.get("lastClr")) {if rgb.len()==6&&rgb.chars().all(|v|v.is_ascii_hexdigit()){out[i]=format!("#{rgb}");}}}}
    });out
}
pub(crate) fn base_style() -> CellStyle { CellStyle { font_size: 11., font_name: "Calibri".into(),color: "#000000".into(),vertical_align: "bottom".into(),number_format: "General".into(), ..Default::default() } }
fn styles(value: &str, theme: &[String]) -> Result<Vec<CellStyle>> {
    let mut indexed=indexed_palette();let mut indexed_section=false;let mut custom=Vec::new();
    visit(value,|tag,a,end| {if tag=="indexedColors"{indexed_section=!end;}if !end&&indexed_section&&tag=="rgbColor"{let c=colour(a,theme,&indexed);if !c.is_empty(){custom.push(c);}}})?;
    if !custom.is_empty(){indexed=custom;}
    let mut fonts = Vec::new(); let mut fills = Vec::new(); let mut borders = Vec::new(); let mut formats = HashMap::new(); let mut out = Vec::<CellStyle>::new();
    let mut font = None::<CellStyle>; let mut fill = None::<String>; let mut border = None::<Vec<String>>; let mut side = None::<usize>;
    let mut cell_xfs = false;
    let mut solid_fill=false;
    visit(value, |tag, a, end| {
        if end {
            match tag {
                "font" => { if let Some(v) = font.take() { fonts.push(v); } },
                "fill" => { if let Some(v) = fill.take() { fills.push(v); } },
                "border" => { if let Some(v) = border.take() { borders.push(v); } side = None; },
                "cellXfs" => cell_xfs = false,
                _ => {}
            }
            return;
        }
        match tag {
            "font" => font = Some(base_style()),
            "b" if font.is_some() => font.as_mut().unwrap().bold = a.get("val").is_none_or(|v| v != "0"),
            "i" if font.is_some() => font.as_mut().unwrap().italic = a.get("val").is_none_or(|v| v != "0"),
            "sz" if font.is_some() => font.as_mut().unwrap().font_size = a.get("val").and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(11.).clamp(1., 409.),
            "color" if font.is_some() => { let c = colour(a,theme,&indexed); if !c.is_empty() { font.as_mut().unwrap().color = c; } },
            "color" if border.is_some() && side.is_some() => {
                let c = colour(a,theme,&indexed); let b = &mut border.as_mut().unwrap()[side.unwrap()];
                if !c.is_empty() && !b.is_empty() { if let Some((prefix,_)) = b.rsplit_once(' ') { *b = format!("{prefix} {c}"); } }
            },
            "name" if font.is_some() => font.as_mut().unwrap().font_name = a.get("val").cloned().unwrap_or_else(|| "Calibri".into()),
            "u" if font.is_some() => font.as_mut().unwrap().underline = a.get("val").is_none_or(|v| v != "none"),
            "strike" if font.is_some() => font.as_mut().unwrap().strike = a.get("val").is_none_or(|v| v != "0"),
            "fill" => {fill = Some(String::new());solid_fill=false;},
            "patternFill" => solid_fill=a.get("patternType").is_some_and(|v|v=="solid"),
            "fgColor" if fill.is_some() && solid_fill => fill = Some(colour(a,theme,&indexed)),
            "border" => border = Some(vec![String::new(); 4]),
            "left" | "right" | "top" | "bottom" if border.is_some() => {
                let i = ["left", "right", "top", "bottom"].iter().position(|v| *v == tag).unwrap(); side = Some(i);
                if let Some(line) = a.get("style") {
                    let (width, kind) = match line.as_str() { "double" => (3, "double"), "medium" | "thick" => (2, "solid"), "dashed" | "dashDot" => (1, "dashed"), "dotted" => (1, "dotted"), _ => (1, "solid") };
                    border.as_mut().unwrap()[i] = format!("{width}px {kind} #64748b");
                }
            },
            "numFmt" => { if let Some(v) = a.get("formatCode") { formats.insert(index(a,"numFmtId"),v.clone()); } },
            "cellXfs" => cell_xfs = true,
            "xf" if cell_xfs => {
                let mut s = fonts.get(index(a,"fontId")).cloned().unwrap_or_else(base_style);
                s.background = fills.get(index(a,"fillId")).cloned().unwrap_or_default();
                s.borders = borders.get(index(a,"borderId")).cloned().unwrap_or_default();
                s.number_format = formats.get(&index(a,"numFmtId")).cloned().unwrap_or_else(|| format!("@builtin:{}",index(a,"numFmtId"))); out.push(s);
            },
            "alignment" if cell_xfs => if let Some(s) = out.last_mut() {
                s.align = a.get("horizontal").filter(|v| ["left", "center", "right"].contains(&v.as_str())).cloned().unwrap_or_default();
                s.wrap = a.get("wrapText").is_some_and(|v| v == "1");
                s.vertical_align = a.get("vertical").cloned().unwrap_or_else(|| "bottom".into());
                s.rotation = a.get("textRotation").and_then(|v| v.parse().ok()).unwrap_or(0);
                s.indent = index(a,"indent");
            },
            _ => {}
        }
    })?;
    if out.is_empty() { out.push(base_style()); }
    Ok(out)
}
pub fn range(value: &str) -> Option<SheetRange> {
    let mut parts = value.split(':'); let a = reader::parse_address(parts.next()?).ok()?; let b = reader::parse_address(parts.next().unwrap_or(value)).ok()?;
    Some(SheetRange { start_row: a.0.min(b.0), end_row: a.0.max(b.0), start_column: a.1.min(b.1), end_column: a.1.max(b.1) })
}
pub fn read(path: &Path, sheet_name: &str, width: usize, offset: usize, limit: usize) -> Result<SheetAppearance> {
    read_internal(path,sheet_name,width,offset,limit,false)
}
pub(crate) fn padded_cells(path:&Path,sheet_name:&str,width:usize)->Result<HashMap<(usize,usize),usize>> {
    let look=read_internal(path,sheet_name,width,0,usize::MAX,true)?;
    Ok(look.cells.iter().filter_map(|c| { let f=&look.styles.get(c.style)?.number_format;
        zero_width(f).map(|n|((c.row,c.column),n))
    }).collect())
}
fn zero_width(s:&str)->Option<usize> { if s.len()>1 && s.len()<=40 && s.chars().all(|v|v=='0'){Some(s.len())}else{None} }
fn read_internal(path: &Path, sheet_name: &str, width: usize, offset: usize, limit: usize, only_padded:bool) -> Result<SheetAppearance> {
    let mut out = SheetAppearance { widths: vec![64.; width],default_row_height:20.,show_grid_lines:true, ..Default::default() };
    if path.extension().and_then(|v| v.to_str()).is_none_or(|v| !v.eq_ignore_ascii_case("xlsx")) {
        if path.extension().and_then(|v| v.to_str()).is_some_and(|v| v.eq_ignore_ascii_case("xls")) {
            let mut v=crate::xls_appearance::read(path,sheet_name,width,offset,if only_padded{limit}else{limit.min(1000)})?;
            if only_padded {v.cells.retain(|c|v.styles.get(c.style).is_some_and(|s|zero_width(&s.number_format).is_some()));}return Ok(v);
        }
        return Ok(out);
    }
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|e| AppError::new("LAYOUT", e.to_string()))?;
    let workbook = xml(&mut zip, "xl/workbook.xml")?; let mut relation = String::new();
    visit(&workbook, |tag,a,end| {
        if !end && tag == "sheet" && a.get("name").is_some_and(|v| v == sheet_name) { relation = a.get("id").cloned().unwrap_or_default(); }
        if !end && tag == "workbookPr" { out.date_1904=a.get("date1904").is_some_and(|v|v=="1"||v=="true"); }
    })?;
    let relations = xml(&mut zip, "xl/_rels/workbook.xml.rels")?; let mut target = String::new();
    visit(&relations, |tag,a,end| { if !end && tag == "Relationship" && a.get("Id").is_some_and(|v| *v == relation) && a.get("TargetMode").is_none_or(|v| v != "External") { target = a.get("Target").cloned().unwrap_or_default(); } })?;
    let target = if target.starts_with('/') { target.trim_start_matches('/').to_string() } else { format!("xl/{target}") };
    let mut parts = Vec::new(); for p in target.split('/') { match p { "." | "" => {}, ".." => { parts.pop(); }, _ => parts.push(p) } }
    let sheet_xml = xml(&mut zip, &parts.join("/"))?;
    let theme=theme(&xml(&mut zip,"xl/theme/theme1.xml").unwrap_or_default());
    out.styles = xml(&mut zip, "xl/styles.xml").and_then(|v| styles(&v,&theme)).unwrap_or_else(|_| vec![base_style()]);
    let mut r=Reader::from_str(&sheet_xml);let mut selected=None::<usize>;let mut numeric=false;let mut in_value=false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e))|Ok(Event::Empty(e)) => {
                let name=e.local_name();let tag=String::from_utf8_lossy(name.as_ref());
                if tag=="v" { in_value=selected.is_some()&&numeric;continue; }
                if !["row","col","c","mergeCell","sheetFormatPr","sheetView"].contains(&tag.as_ref()) { continue; }
                let a=attrs(&e,r.decoder());
                match tag.as_ref() {
                    "sheetView" => out.show_grid_lines=a.get("showGridLines").is_none_or(|v|v!="0"&&v!="false"),
                    "sheetFormatPr" => {
                        out.default_row_height=a.get("defaultRowHeight").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()).map(|v|(v*4./3.).clamp(1.,546.)).unwrap_or(20.);
                        if let Some(w)=a.get("defaultColWidth").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()) { out.widths.fill((w*7.+5.).clamp(1.,2500.)); }
                    },
                    "row" if offset==0 => {
                        if a.get("hidden").is_some_and(|v|v=="1"||v=="true") { out.row_heights.insert(index(&a,"r"),0.); }
                        else if let Some(h)=a.get("ht").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()) { out.row_heights.insert(index(&a,"r"),(h*4./3.).clamp(1.,546.)); }
                    },
                    "col" => {
                        let px=if a.get("hidden").is_some_and(|v|v=="1"||v=="true") { 0. } else { a.get("width").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()).map(|v|(v*7.+5.).clamp(1.,2500.)).unwrap_or(64.) };
                        for c in index(&a,"min").max(1)..=index(&a,"max").min(width) {
                            out.widths[c-1]=px;
                            if let Some(w)=a.get("width").and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite()&&*v>=0.) {out.column_widths.insert(c,w);}
                        }
                    },
                    "c" => {
                        selected=None;in_value=false;numeric=a.get("t").is_none_or(|v|v=="n");
                        if let Some((row,column))=a.get("r").and_then(|v|reader::parse_address(v).ok()) {
                            if row>offset && row<=offset.saturating_add(if only_padded{limit}else{limit.min(1000)}) && column<=width
                                && (!only_padded || out.styles.get(index(&a,"s")).is_some_and(|s|zero_width(&s.number_format).is_some())) {
                                out.cells.push(CellAppearance { row,column,style:index(&a,"s"),raw_number:None });selected=Some(out.cells.len()-1);
                            }
                        }
                    },
                    "mergeCell" => if let Some(v)=a.get("ref").and_then(|v|range(v)) { if out.merges.len()<100_000 { out.merges.push(v); } },
                    _=>{}
                }
            },
            Ok(Event::Text(e)) if in_value => if let Some(i)=selected {
                out.cells[i].raw_number=e.decode().ok().and_then(|v|v.parse::<f64>().ok()).filter(|v|v.is_finite());
            },
            Ok(Event::End(e)) if e.local_name().as_ref()==b"v" => in_value=false,
            Ok(Event::Eof)=>break,
            Err(e)=>return Err(AppError::new("LAYOUT",e.to_string())),
            _=>{}
        }
    }
    Ok(out)
}
