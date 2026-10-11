//! BIFF8 presentation records; cell values are read by Calamine, independently.
use crate::{appearance::{base_style,indexed_palette},model::*};
use std::{collections::HashMap,io::Read,path::Path};
fn u16_at(p:&[u8],n:usize)->usize { p.get(n..n+2).map(|v|u16::from_le_bytes([v[0],v[1]]) as usize).unwrap_or(0) }
fn u32_at(p:&[u8],n:usize)->u32 { p.get(n..n+4).map(|v|u32::from_le_bytes(v.try_into().unwrap())).unwrap_or(0) }
fn text(p:&[u8],start:usize,len:usize,wide:bool)->String {
    if wide { String::from_utf16_lossy(&p.get(start..start.saturating_add(len*2)).unwrap_or(&[]).chunks_exact(2).map(|v|u16::from_le_bytes([v[0],v[1]])).collect::<Vec<_>>()) }
    else { p.get(start..start.saturating_add(len)).unwrap_or(&[]).iter().map(|v|char::from(*v)).collect() }
}
fn records(p:&[u8],start:usize)->Vec<(u16,&[u8])> {
    let mut out=Vec::new();let mut pos=start;
    while pos+4<=p.len() { let tag=u16_at(p,pos) as u16;let end=pos+4+u16_at(p,pos+2);if end>p.len(){break;}out.push((tag,&p[pos+4..end]));pos=end;if tag==0x0a{break;} }
    out
}
fn color(palette:&[String],index:usize)->String { palette.get(index).cloned().unwrap_or_else(|| "#000000".into()) }
fn border(kind:usize,c:&str)->String {
    match kind {0=>String::new(),2|5=>format!("2px solid {c}"),3|7|9|10=>format!("1px dashed {c}"),4=>format!("1px dotted {c}"),6=>format!("3px double {c}"),_=>format!("1px solid {c}")}
}
fn rk(raw:u32)->f64 { let n=if raw&2!=0 {(raw as i32>>2) as f64}else{f64::from_bits(((raw&!3) as u64)<<32)};if raw&1!=0 {n/100.}else{n} }
pub fn read(path:&Path,sheet_name:&str,width:usize,offset:usize,limit:usize)->Result<SheetAppearance> {
    let mut c=cfb::open(path)?;
    let stream=c.walk().find(|v|v.is_stream()&&v.path().file_name().is_some_and(|n|n.to_string_lossy().eq_ignore_ascii_case("Workbook")||n.to_string_lossy().eq_ignore_ascii_case("Book"))).map(|v|v.path().to_path_buf()).ok_or_else(||AppError::new("LAYOUT","找不到 XLS 工作簿流"))?;
    let mut bytes=Vec::new();c.open_stream(stream)?.take(80_000_001).read_to_end(&mut bytes)?;
    if bytes.len()>80_000_000{return Err(AppError::new("LAYOUT","XLS 格式数据较大，无法预览原始格式"));}
    let mut fonts=Vec::<CellStyle>::new();let mut xfs=Vec::new();let mut formats=HashMap::new();let mut palette=indexed_palette();let mut sheet_offset=None;let mut date_1904=false;
    for (tag,p) in records(&bytes,0) {
        match tag {
            0x85 if p.len()>=8 => { let name=text(p,8,p[6] as usize,p[7]&1!=0);if name==sheet_name{sheet_offset=Some(u32_at(p,0) as usize);} },
            0x31 if p.len()>=16 => {
                if fonts.len()==4{fonts.push(base_style());}
                let mut f=base_style();f.font_size=u16_at(p,0) as f64/20.;f.bold=u16_at(p,6)>=700;f.italic=u16_at(p,2)&2!=0;f.strike=u16_at(p,2)&8!=0;f.underline=p[10]!=0;
                f.color=format!("@indexed:{}",u16_at(p,4));f.font_name=text(p,16,p[14] as usize,p[15]&1!=0);fonts.push(f);
            },
            0xe0 if p.len()>=20=>xfs.push(p.to_vec()),
            0x41e if p.len()>=5=>{formats.insert(u16_at(p,0),text(p,5,u16_at(p,2),p[4]&1!=0));},
            0x92 if p.len()>=2=>for (i,rgb) in p[2..].chunks_exact(4).take(u16_at(p,0)).enumerate(){if 8+i<palette.len(){palette[8+i]=format!("#{:02X}{:02X}{:02X}",rgb[0],rgb[1],rgb[2]);}},
            0x22=>date_1904=u16_at(p,0)!=0,
            _=>{}
        }
    }
    let mut out=SheetAppearance { widths:vec![64.;width],default_row_height:20.,date_1904,show_grid_lines:true,..Default::default() };
    for p in xfs {
        let mut s=fonts.get(u16_at(&p,0)).cloned().unwrap_or_else(base_style);
        if let Some(i)=s.color.strip_prefix("@indexed:").and_then(|v|v.parse().ok()){s.color=color(&palette,i);}
        s.number_format=formats.get(&u16_at(&p,2)).cloned().unwrap_or_else(||format!("@builtin:{}",u16_at(&p,2)));
        s.align=match p[6]&7{1=>"left",2|6=>"center",3=>"right",5|7=>"justify",_=>""}.into();s.wrap=p[6]&8!=0;
        s.vertical_align=match (p[6]>>4)&7{0=>"top",1=>"center",_=>"bottom"}.into();s.rotation=p[7] as i32;s.indent=(p[8]&15) as usize;
        let b1=u32_at(&p,10);let b2=u32_at(&p,14);
        s.borders=vec![border((b1&15) as usize,&color(&palette,((b1>>16)&127) as usize)),border(((b1>>4)&15) as usize,&color(&palette,((b1>>23)&127) as usize)),border(((b1>>8)&15) as usize,&color(&palette,(b2&127) as usize)),border(((b1>>12)&15) as usize,&color(&palette,((b2>>7)&127) as usize))];
        if (b2>>26)&63==1 {s.background=color(&palette,u16_at(&p,18)&127);}
        out.styles.push(s);
    }
    if out.styles.is_empty(){out.styles.push(base_style());}
    let start=sheet_offset.ok_or_else(||AppError::new("LAYOUT","XLS 工作表不存在"))?;
    for (tag,p) in records(&bytes,start) {
        match tag {
            0x23e if p.len()>=2=>out.show_grid_lines=u16_at(p,0)&2!=0,
            0x225 if p.len()>=4=>out.default_row_height=u16_at(p,2) as f64/15.,
            0x55 if p.len()>=2=>out.widths.fill(u16_at(p,0) as f64*7.+5.),
            0x99 if p.len()>=2=>out.widths.fill(u16_at(p,0) as f64/256.*7.+5.),
            0x7d if p.len()>=10=> {
                let px=if u16_at(p,8)&1!=0 {0.}else{u16_at(p,4) as f64/256.*7.+5.};
                for i in u16_at(p,0)..=u16_at(p,2).min(width.saturating_sub(1)){out.widths[i]=px;}
            },
            0x208 if p.len()>=16 && offset==0=>{let height=if u32_at(p,12)&32!=0{0.}else{(u16_at(p,6)&0x7fff) as f64/15.};if height>0.||u32_at(p,12)&32!=0{out.row_heights.insert(u16_at(p,0)+1,height);}},
            0xbd|0xbe if p.len()>=6=> {
                let row=u16_at(p,0)+1;let step=if tag==0xbd{6}else{2};
                if row>offset&&row<=offset.saturating_add(limit){for (i,value) in p[4..p.len()-2].chunks_exact(step).enumerate(){let column=u16_at(p,2)+i+1;if column<=width{out.cells.push(CellAppearance{row,column,style:u16_at(value,0),raw_number:if tag==0xbd{Some(rk(u32_at(value,2)))}else{None}});}}}
            },
            0x203|0x27e|0xfd|0x201|0x204|0x205|0x06 if p.len()>=6=> {
                let row=u16_at(p,0)+1;let column=u16_at(p,2)+1;
                if row>offset&&row<=offset.saturating_add(limit)&&column<=width{
                    let n=match tag{0x203|0x06 if p.len()>=14=>Some(f64::from_le_bytes(p[6..14].try_into().unwrap())),0x27e if p.len()>=10=>Some(rk(u32_at(p,6))),_=>None};
                    out.cells.push(CellAppearance{row,column,style:u16_at(p,4),raw_number:n.filter(|v|v.is_finite())});
                }
            },
            _=>{}
        }
    }
    let book:calamine::Xls<std::io::BufReader<std::fs::File>>=calamine::open_workbook(path).map_err(|e:calamine::XlsError|AppError::new("LAYOUT",e.to_string()))?;
    out.merges=book.worksheet_merge_cells(sheet_name).unwrap_or_default().into_iter().map(|r|SheetRange{start_row:r.start.0 as usize+1,end_row:r.end.0 as usize+1,start_column:r.start.1 as usize+1,end_column:r.end.1 as usize+1}).collect();
    Ok(out)
}
