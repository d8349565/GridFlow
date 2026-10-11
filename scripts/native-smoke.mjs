import {spawn} from 'node:child_process';
import {readFile,writeFile,mkdir,readdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';

const root=process.cwd();const app=path.join(root,'src-tauri/target/release/excel-workbench.exe');
const data=path.join(root,'.cache/native-smoke-'+Date.now());await mkdir(data,{recursive:true});await mkdir(path.join(root,'.cache/ux-review'),{recursive:true});
const env={...process.env,GRIDFLOW_DATA_DIR:data,EXCEL_WORKBENCH_DATA_DIR:data,PATH:path.join(root,'.tools/duckdb')+';'+process.env.PATH};
const driver=spawn(path.join(root,'.tools/tauri-driver/bin/tauri-driver.exe'),['--native-driver',path.join(root,'.tools/edgedriver/msedgedriver.exe'),'--port','4444','--native-port','4445'],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
let log='';driver.stdout.on('data',b=>{log+=b});driver.stderr.on('data',b=>{log+=b});
const base='http://127.0.0.1:4444';const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function request(url,method='GET',body){const response=await fetch(base+url,{method,headers:body?{'Content-Type':'application/json'}:undefined,body:body?JSON.stringify(body):undefined});const value=await response.json();if(value.value?.error)throw Error(JSON.stringify(value));return value.value;}
let session;
try{
  for(let i=0;i<100;i++){try{await request('/status');break}catch{await delay(100)}}
  const started=performance.now();const created=await request('/session','POST',{capabilities:{alwaysMatch:{'tauri:options':{application:app}}}});session=created.sessionId;
  async function js(script,args=[]){return request(`/session/${session}/execute/sync`,'POST',{script,args})}
  async function ipc(command,args={}){return request(`/session/${session}/execute/async`,'POST',{script:'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(v=>done({ok:true,value:v}),e=>done({ok:false,error:e}));',args:[command,args]}).then(r=>{if(!r.ok)throw Error(JSON.stringify(r.error));return r.value})}
  for(let i=0;i<100;i++){if(await js('return !!document.querySelector(".app-shell")'))break;await delay(100)}
  const startupMs=performance.now()-started;
  assert.match(await js('return document.body.textContent'),/汇格 · GridFlow/);
  // IPC imports bypass the UI's import action. Let initial autosave finish first
  // so it cannot overwrite the test draft while the backend imports files.
  for(let i=0;i<100;i++){if(await js('return performance.getEntriesByType("resource").some(e=>e.name.endsWith("/save_draft"))'))break;await delay(100)}
  assert(await js('return performance.getEntriesByType("resource").some(e=>e.name.endsWith("/save_draft"))'),"initial workspace must settle before direct IPC imports");
  const sampleRoot=process.env.WORKBENCH_SMOKE_INPUT_DIR || path.join(root,'tests/fixtures/excel');const samples=(await readdir(sampleRoot)).filter(name=>name.endsWith('.xlsx')).map(name=>path.join(sampleRoot,name));
  const imported=await ipc('import_files',{paths:samples});assert.equal(imported.length,process.env.WORKBENCH_SMOKE_INPUT_DIR ? samples.length : 5);assert(imported.every(item=>!item.error));
  const ws=await ipc('workspace');assert.equal(path.normalize(ws.dataDirectory),path.normalize(data),"native test must use isolated data root");const source=ws.files[0].sheets[0];let rules=ws.rules;
  rules.sources=rules.sources.map(s=>({...s,enabled:process.env.WORKBENCH_SMOKE_INPUT_DIR ? true : s.sheetId===source.id}));
  if(process.env.WORKBENCH_SMOKE_HEADER_ROW) rules.sources=rules.sources.map(s=>({...s,headerRow:Number(process.env.WORKBENCH_SMOKE_HEADER_ROW)}));
  if(process.env.WORKBENCH_SMOKE_HEADER_RANGE) {
    const area=JSON.parse(process.env.WORKBENCH_SMOKE_HEADER_RANGE);rules.sources=rules.sources.map(s=>({...s,headerRow:area.endRow,headerRange:area}));
    rules.fixedCells=[{id:'native-shop',name:'门店名称',sheetName:source.name,address:'B2'}];
  }
  const hints=await ipc('suggestions',{rules});assert(hints.every(h=>!h.name.startsWith('input:')));
  if(!process.env.WORKBENCH_SMOKE_INPUT_DIR) rules.columns=hints.map(h=>({key:h.key,name:h.name,dataType:h.suggestedType,hidden:false}));
  await ipc('save_draft',{rules,activeSheetId:source.id});await js('location.reload();return true');
  for(let i=0;i<100;i++){if(await js('return document.querySelectorAll(".sheet-item").length>0'))break;await delay(100)}
  for(let i=0;i<100;i++){if(await js('return document.querySelector(".excel-cell[data-cell=A1]")?.textContent.length>0'))break;await delay(100)}
  assert(await js('return !!document.querySelector(".excel-scroll")'),"saved active sheet must restore the original Excel preview");
  assert(await js('return !!document.querySelector(".rules-panel") && !document.querySelector(".pane-tabs")'), "configuration must stay focused on the original sheet");
  await delay(250);
  const layout=await js('const r=document.querySelector(".excel-scroll").getBoundingClientRect();return {viewport:{width:innerWidth,height:innerHeight},excel:{x:r.x,y:r.y,width:r.width,height:r.height}}');
  assert(layout.excel.height>=layout.viewport.height*0.7,"original Excel must get most of the available height");
  const originalFont=await js('return getComputedStyle(document.querySelector(".excel-cell[data-cell=A1]")).fontSize');
  await js('document.querySelector("button[aria-label=专注表格]").click();return true');
  const focus=await js('return {width:document.querySelector(".excel-scroll").getBoundingClientRect().width,panels:document.querySelectorAll(".file-sidebar,.rules-panel").length,font:getComputedStyle(document.querySelector(".excel-cell[data-cell=A1]")).fontSize}');
  assert.equal(focus.panels,0);assert.equal(focus.width,layout.viewport.width);assert.equal(focus.font,originalFont);
  const focusScreenshot=await request(`/session/${session}/screenshot`);await writeFile(path.join(root,'.cache/ux-review/07-focus.png'),Buffer.from(focusScreenshot,'base64'));
  await js('document.querySelector("button[aria-label=退出专注]").click();return true');
  assert(await js('return document.querySelectorAll(".file-sidebar,.rules-panel").length===2'),"focus mode must restore both sidebars");
  if(process.env.WORKBENCH_SMOKE_HEADER_RANGE) {
    const original=await js('return {titleFont:getComputedStyle(document.querySelector(".excel-cell[data-cell=A1]")).fontSize,code:document.querySelector(".excel-cell[data-cell=B6]").textContent,amount:document.querySelector(".excel-cell[data-cell=C6]").textContent,percent:document.querySelector(".excel-cell[data-cell=D6]").textContent}');
    assert(parseFloat(original.titleFont)>29);assert.equal(original.code,'00023');assert.match(original.amount,/1,234\.50/);assert.equal(original.percent,'12.5%');
  }
  const sourceScreenshot=await request(`/session/${session}/screenshot`);await writeFile(path.join(root,'.cache/ux-review/03-new-workbench.png'),Buffer.from(sourceScreenshot,'base64'));
  const sourceList=await js('const rows=Array.from(document.querySelectorAll(".source-file-row"));const viewport=document.querySelector(".file-scroll").getBoundingClientRect();return {pageHeight:document.documentElement.scrollHeight,pageWidth:document.documentElement.clientWidth,files:rows.length,rowHeight:rows[0].getBoundingClientRect().height,visibleFiles:rows.filter(row=>{const r=row.getBoundingClientRect();return r.top>=viewport.top&&r.bottom<=viewport.bottom}).length,common:document.querySelector(".source-file-caption")?.textContent,firstLabel:rows[0].querySelector(".source-file-preview").textContent,firstFullName:rows[0].querySelector(".source-file-preview").getAttribute("title"),repeatedMetadata:document.querySelectorAll(".file-meta,.sidebar-bottom").length}');
  assert(sourceList.rowHeight<=38);assert.equal(sourceList.repeatedMetadata,0);
  assert(sourceList.pageHeight<=layout.viewport.height+1);assert.equal(sourceList.pageWidth,layout.viewport.width);
  const sidebar=await request(`/session/${session}/element`,'POST',{using:'css selector',value:'.file-sidebar'});
  const sidebarId=sidebar['element-6066-11e4-a52e-4f735466cecf'];
  const sidebarScreenshot=await request(`/session/${session}/element/${sidebarId}/screenshot`);await writeFile(path.join(root,'.cache/ux-review/08-files.png'),Buffer.from(sidebarScreenshot,'base64'));
  await js('Array.from(document.querySelectorAll("button")).find(b=>b.textContent.trim()==="格式建议").click();return true');
  await delay(100);
  for(let i=0;i<100;i++){if(await js('return document.querySelector("dialog button.primary")?.disabled === false && !document.querySelector("dialog").textContent.includes("正在检查")'))break;await delay(100)}
  assert(await js('return document.querySelector("dialog h2")?.textContent==="合并前检查"'));
  assert(await js('return !document.querySelector(".suggestion-examples") && !document.querySelector(".suggestion-list")'), "unchanged suggestions must not create repetitive tables");
  const checkScreenshot=await request(`/session/${session}/screenshot`);await writeFile(path.join(root,'.cache/ux-review/04-new-check.png'),Buffer.from(checkScreenshot,'base64'));
  await js('Array.from(document.querySelectorAll("dialog button")).find(b=>b.textContent.trim()==="按当前设置合并").click();return true');
  let completed;
  for(let i=0;i<200;i++){const tasks=await ipc('tasks');completed=tasks.find(t=>t.state==='succeeded');if(completed)break;if(tasks.some(t=>t.state==='failed'))throw Error(JSON.stringify(tasks));await delay(100)}
  assert(completed);const current=await ipc('workspace');const dataset=current.datasets.find(d=>d.id===completed.datasetId);assert(dataset.summary.outputRows>0);
  if(process.env.WORKBENCH_SMOKE_INPUT_DIR) { assert.equal(dataset.summary.outputRows,Number(process.env.WORKBENCH_SMOKE_EXPECTED_ROWS || 960));assert.equal(dataset.rules.columns.length,0,"no optional formats may be applied implicitly"); }
  const query={datasetId:dataset.id,offset:0,limit:100,sorts:[],filters:[],search:''};const page=await ipc('query_dataset',{query});assert(page.rows.length>0);assert(page.columns.every(c=>!c.name.startsWith("input:")),"internal field IDs must not leak into labels");
  if(process.env.WORKBENCH_SMOKE_HEADER_RANGE) {
    const code=page.columns.findIndex(c=>c.name==='商品编号');const shop=page.columns.findIndex(c=>c.name==='门店名称');
    assert.equal(page.rows[0][code].value,'00023');
    assert(page.rows.some(r=>r[shop].value==='上海店')&&page.rows.some(r=>r[shop].value==='北京店'));
  }
  const exported=path.join(data,'真实桌面导出.xlsx');await ipc('export_dataset',{query,path:exported});assert((await readFile(exported)).length>1000);
  for(let i=0;i<100;i++){if(await js('return document.querySelector(".pane-tabs .active")?.textContent.includes("汇总表")'))break;await delay(100)}
  assert(await js('return document.querySelector(".pane-tabs .active")?.textContent.includes("汇总表")'),"completed task must automatically show results");
  await delay(300);
  assert(await js('return !document.querySelector(".file-sidebar") && !document.querySelector(".rules-panel")'), "results must not include source selection or configuration controls");
  for(let i=0;i<100;i++){if(await js('return Array.from(document.querySelectorAll(".ag-cell")).some(el=>el.textContent.trim().length>1 && el.textContent.trim()!=="—")'))break;await delay(100)}
  const screenshot=await request(`/session/${session}/screenshot`);await writeFile(path.join(root,'.cache/native-workbench.png'),Buffer.from(screenshot,'base64'));await writeFile(path.join(root,'.cache/ux-review/05-new-result.png'),Buffer.from(screenshot,'base64'));
  await js('Array.from(document.querySelectorAll(".pane-tabs button")).find(b=>b.textContent.trim()==="分类统计").click();return true');await delay(250);
  assert(await js('return Array.from(document.querySelectorAll(".analysis-choice strong")).some(el=>el.textContent.includes("按什么分类"))'));
  assert(await js('return document.querySelector(".stage-nav [aria-current=step]")?.textContent.includes("预览与保存")'), "analysis must keep the completed workflow state");
  assert(await js('return Array.from(document.querySelectorAll(".app-header button")).some(b=>b.getAttribute("aria-label")==="返回修改配置")'), "analysis must provide a clear route back to save the merged table");
  const analysisScreenshot=await request(`/session/${session}/screenshot`);await writeFile(path.join(root,'.cache/ux-review/06-new-analysis.png'),Buffer.from(analysisScreenshot,'base64'));
  const resources=await js('return performance.getEntriesByType("resource").map(e=>e.name)');assert(resources.every(url=>!/https:\/\//.test(url)),JSON.stringify(resources));
  const report={startupMs,importedFiles:imported.length,outputRows:dataset.summary.outputRows,exports:exported,layout,focus,sourceList,resourceUrls:resources,os:'Windows 11 x64',dataDirectory:ws.dataDirectory};await writeFile(path.join(root,'.cache/native-smoke.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
}finally{
  if(session){try{await request(`/session/${session}`,'DELETE')}catch{}}
  driver.kill();await writeFile(path.join(root,'.cache/native-driver.log'),log);
}
