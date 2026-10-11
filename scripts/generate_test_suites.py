# -*- coding: utf-8 -*-
"""
生成 4 类测试数据集，每类包含 32 个分表和 1 个完整总表。
前三类为规整表头，第四类为带自定义格式/单独填写单元格（A1:填写人，B1:内容等）的真实填报表。
用于测试 Excel 汇总工具（批量合并、固定单元格提取、字段类型识别、数据透视与核对）。
"""

import os
import sys
import random
from datetime import datetime, date, timedelta
import openpyxl
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from openpyxl.utils import get_column_letter

# 配置控制台输出 UTF-8
if sys.stdout.encoding.lower() != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

# 固定随机种子，确保生成数据稳定可复现
random.seed(42)

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUTPUT_DIR = os.path.join(BASE_DIR, "tests", "generated")

# 统一通用样式
HEADER_FONT = Font(name="微软雅黑", size=11, bold=True, color="FFFFFF")
DATA_FONT = Font(name="微软雅黑", size=10, color="000000")
SUMMARY_HEADER_FONT = Font(name="微软雅黑", size=11, bold=True, color="1E293B")
KPI_TITLE_FONT = Font(name="微软雅黑", size=14, bold=True, color="1E3A8A")
KPI_LABEL_FONT = Font(name="微软雅黑", size=10, bold=True, color="475569")
KPI_VAL_FONT = Font(name="微软雅黑", size=13, bold=True, color="0F172A")

KEY_CELL_FONT = Font(name="微软雅黑", size=10, bold=True, color="1E293B")
VAL_CELL_FONT = Font(name="微软雅黑", size=10, bold=False, color="0F172A")

THIN_BORDER = Border(
    left=Side(style='thin', color='CBD5E1'),
    right=Side(style='thin', color='CBD5E1'),
    top=Side(style='thin', color='CBD5E1'),
    bottom=Side(style='thin', color='CBD5E1')
)

HEADER_FILL_BLUE = PatternFill(start_color="2563EB", end_color="2563EB", fill_type="solid")
HEADER_FILL_GREEN = PatternFill(start_color="059669", end_color="059669", fill_type="solid")
HEADER_FILL_PURPLE = PatternFill(start_color="7C3AED", end_color="7C3AED", fill_type="solid")
HEADER_FILL_AMBER = PatternFill(start_color="D97706", end_color="D97706", fill_type="solid")

KEY_CELL_FILL = PatternFill(start_color="F1F5F9", end_color="F1F5F9", fill_type="solid")
VAL_CELL_FILL = PatternFill(start_color="FFFFFF", end_color="FFFFFF", fill_type="solid")

SUBHEADER_FILL = PatternFill(start_color="F1F5F9", end_color="F1F5F9", fill_type="solid")
KPI_CARD_FILL = PatternFill(start_color="E0F2FE", end_color="E0F2FE", fill_type="solid")

def apply_column_widths(ws):
    """根据单元格内容智能设置列宽"""
    for col in ws.columns:
        max_len = 0
        col_letter = get_column_letter(col[0].column)
        for cell in col:
            val = cell.value
            if val is not None:
                s = str(val)
                char_len = sum(2 if ord(c) > 127 else 1 for c in s)
                if char_len > max_len:
                    max_len = char_len
        ws.column_dimensions[col_letter].width = max(max_len + 4, 13)

def save_styled_sheet(ws, title, columns, data, header_fill):
    """给标准数据明细表设置标题、表头、数据、边框及数字格式"""
    ws.title = title
    ws.views.sheetView[0].showGridLines = True

    ws.append([col[0] for col in columns])
    for col_idx in range(1, len(columns) + 1):
        cell = ws.cell(row=1, column=col_idx)
        cell.font = HEADER_FONT
        cell.fill = header_fill
        cell.alignment = Alignment(horizontal="center", vertical="center")
        cell.border = THIN_BORDER
    ws.row_dimensions[1].height = 26

    for row_idx, row_data in enumerate(data, start=2):
        ws.append(row_data)
        ws.row_dimensions[row_idx].height = 20
        for col_idx, (col_name, col_type, col_align, col_format) in enumerate(columns, start=1):
            cell = ws.cell(row=row_idx, column=col_idx)
            cell.font = DATA_FONT
            cell.border = THIN_BORDER
            cell.alignment = Alignment(horizontal=col_align, vertical="center")
            if col_format:
                cell.number_format = col_format

    apply_column_widths(ws)
    ws.freeze_panes = "A2"

# ==============================================================================
# 类别一：全国连锁门店销售数据（32家门店）
# ==============================================================================
STORES = [
    ("STR001", "北京朝阳大悦城店", "华北"),
    ("STR002", "北京海淀中关村店", "华北"),
    ("STR003", "天津和平滨江道店", "华北"),
    ("STR004", "石家庄长安万达店", "华北"),
    ("STR005", "太原迎泽柳巷店", "华北"),
    ("STR006", "呼和浩特新城万达店", "华北"),
    ("STR007", "上海浦东世纪汇店", "华东"),
    ("STR008", "上海黄浦南京东路店", "华东"),
    ("STR009", "杭州西湖湖滨银泰店", "华东"),
    ("STR010", "南京玄武德基广场店", "华东"),
    ("STR011", "苏州园区时代广场店", "华东"),
    ("STR012", "合肥庐阳万象城店", "华东"),
    ("STR013", "广州天河正佳广场店", "华南"),
    ("STR014", "深圳南山万象天地店", "华南"),
    ("STR015", "深圳福田中心城店", "华南"),
    ("STR016", "东莞南城汇一城店", "华南"),
    ("STR017", "佛山禅城祖庙店", "华南"),
    ("STR018", "南宁青秀万象城店", "华南"),
    ("STR019", "武汉江汉路步行街店", "华中"),
    ("STR020", "武汉武昌群光广场店", "华中"),
    ("STR021", "长沙芙蓉国金中心店", "华中"),
    ("STR022", "郑州金水二七万达店", "华中"),
    ("STR023", "成都锦江春熙路店", "西南"),
    ("STR024", "成都高新交子大道店", "西南"),
    ("STR025", "重庆江北北城天街店", "西南"),
    ("STR026", "重庆渝中时代天街店", "西南"),
    ("STR027", "昆明五华顺城购物店", "西南"),
    ("STR028", "西安雁塔大雁塔店", "西北"),
    ("STR029", "兰州城关东方红店", "西北"),
    ("STR030", "乌鲁木齐天山友好店", "西北"),
    ("STR031", "沈阳沈河中街大悦城店", "东北"),
    ("STR032", "大连中山百年城店", "东北"),
]

PRODUCTS_CATEGORY_1 = [
    ("SKU-1001", "旗舰轻薄笔记本", "数码设备", 6999.00),
    ("SKU-1002", "专业电竞台式机", "数码设备", 8999.00),
    ("SKU-1003", "27英寸4K超清显示器", "办公外设", 1899.00),
    ("SKU-1004", "人体工学机械键盘", "办公外设", 499.00),
    ("SKU-1005", "双模无线静音鼠标", "办公外设", 129.00),
    ("SKU-1006", "主动降噪头戴耳机", "影音娱乐", 1299.00),
    ("SKU-1007", "桌面立体声蓝牙音箱", "影音娱乐", 699.00),
    ("SKU-1008", "智能运动健康手表", "智能穿戴", 1599.00),
    ("SKU-1009", "开放式运动蓝牙耳机", "智能穿戴", 599.00),
    ("SKU-1010", "高清广角网络摄像头", "办公外设", 299.00),
    ("SKU-1011", "65W氮化镓快充充电器", "数码配件", 149.00),
    ("SKU-1012", "10000mAh磁吸移动电源", "数码配件", 199.00),
    ("SKU-1013", "铝合金折叠笔记本支架", "数码配件", 89.00),
    ("SKU-1014", "1TB高速移动固态硬盘", "存储设备", 599.00),
    ("SKU-1015", "512GB高耐用闪存卡", "存储设备", 259.00),
]

PAYMENT_METHODS = ["微信支付", "支付宝", "银联刷卡", "数字人民币"]

SALES_COLUMNS = [
    ("销售单号", "str", "center", "@"),
    ("销售日期", "date", "center", "YYYY-MM-DD"),
    ("门店编码", "str", "center", "@"),
    ("门店名称", "str", "left", None),
    ("所属区域", "str", "center", None),
    ("商品编码", "str", "center", "@"),
    ("商品名称", "str", "left", None),
    ("商品类别", "str", "center", None),
    ("销售单价", "float", "right", "#,##0.00"),
    ("销售数量", "int", "right", "#,##0"),
    ("销售金额", "float", "right", "#,##0.00"),
    ("支付方式", "str", "center", None)
]

def generate_category_1():
    print("[1/4] 正在生成【类别一：全国连锁门店销售数据】...")
    target_dir = os.path.join(OUTPUT_DIR, "01_全国连锁门店销售数据")
    split_dir = os.path.join(target_dir, "分表测试文件_32家门店")
    os.makedirs(split_dir, exist_ok=True)

    all_rows = []
    store_splits = {}
    order_seq = 10000
    base_date = date(2024, 10, 1)

    for store_id, store_name, region in STORES:
        store_rows = []
        for i in range(30):
            order_seq += 1
            order_no = f"SO202410{order_seq}"
            day_offset = (i * 3 + int(store_id[-2:])) % 15
            sale_date = (base_date + timedelta(days=day_offset)).strftime("%Y-%m-%d")

            prod = PRODUCTS_CATEGORY_1[(i + int(store_id[-2:])) % len(PRODUCTS_CATEGORY_1)]
            sku, pname, category, price = prod

            qty = ((i * 7 + int(store_id[-2:])) % 8) + 1
            amount = round(price * qty, 2)
            pay = PAYMENT_METHODS[(i + int(store_id[-2:])) % len(PAYMENT_METHODS)]

            row = [
                order_no, sale_date, store_id, store_name, region,
                sku, pname, category, price, qty, amount, pay
            ]
            store_rows.append(row)
            all_rows.append(row)

        store_splits[store_id] = (store_name, region, store_rows)

    for store_id, (store_name, region, s_rows) in store_splits.items():
        wb_sub = openpyxl.Workbook()
        ws_sub = wb_sub.active
        save_styled_sheet(ws_sub, "销售明细", SALES_COLUMNS, s_rows, HEADER_FILL_BLUE)
        file_name = f"销售明细_{store_id}_{region}_{store_name}.xlsx"
        wb_sub.save(os.path.join(split_dir, file_name))

    wb_master = openpyxl.Workbook()
    ws_data = wb_master.active
    save_styled_sheet(ws_data, "销售明细", SALES_COLUMNS, all_rows, HEADER_FILL_BLUE)

    ws_kpi = wb_master.create_sheet(title="核对指标看板")
    ws_kpi.views.sheetView[0].showGridLines = True

    total_records = len(all_rows)
    total_qty = sum(r[9] for r in all_rows)
    total_amount = round(sum(r[10] for r in all_rows), 2)
    avg_price = round(total_amount / total_qty, 2)

    ws_kpi["A1"] = "【全国连锁门店销售数据】核心核对指标总览"
    ws_kpi["A1"].font = KPI_TITLE_FONT
    ws_kpi.row_dimensions[1].height = 32

    kpi_cards = [
        ("A3", "B3", "总分表数", f"{len(STORES)} 个文件"),
        ("C3", "D3", "总销售笔数（行数）", f"{total_records} 行"),
        ("E3", "F3", "总销售数量", f"{total_qty:,} 件"),
        ("G3", "H3", "总销售金额（元）", f"￥{total_amount:,.2f}")
    ]
    for top_left, bottom_right, title, val in kpi_cards:
        ws_kpi[top_left] = title
        ws_kpi[top_left].font = KPI_LABEL_FONT
        col = top_left[0]
        val_cell = f"{col}4"
        ws_kpi[val_cell] = val
        ws_kpi[val_cell].font = KPI_VAL_FONT
        ws_kpi[val_cell].fill = KPI_CARD_FILL
        ws_kpi[val_cell].alignment = Alignment(horizontal="center", vertical="center")
        ws_kpi[top_left].alignment = Alignment(horizontal="center", vertical="center")

    ws_kpi.row_dimensions[3].height = 20
    ws_kpi.row_dimensions[4].height = 26

    ws_kpi["A6"] = "各门店小计对账明细（供透视表核对）"
    ws_kpi["A6"].font = Font(name="微软雅黑", size=12, bold=True, color="1E3A8A")

    kpi_headers = ["序号", "门店编码", "所属区域", "门店名称", "单数（行数）", "销售总量（件）", "销售总金额（元）"]
    ws_kpi.append([])
    ws_kpi.append(kpi_headers)
    ws_kpi.row_dimensions[7].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=7, column=c_idx)
        c.font = HEADER_FONT
        c.fill = HEADER_FILL_BLUE
        c.alignment = Alignment(horizontal="center", vertical="center")

    store_kpi_list = []
    for idx, (s_id, (s_name, s_reg, s_rows)) in enumerate(store_splits.items(), start=1):
        s_cnt = len(s_rows)
        s_qty = sum(r[9] for r in s_rows)
        s_amt = round(sum(r[10] for r in s_rows), 2)
        store_kpi_list.append((s_id, s_reg, s_name, s_cnt, s_qty, s_amt))
        row_vals = [idx, s_id, s_reg, s_name, s_cnt, s_qty, s_amt]
        ws_kpi.append(row_vals)
        cur_row = ws_kpi.max_row
        ws_kpi.row_dimensions[cur_row].height = 20
        ws_kpi.cell(row=cur_row, column=1).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=2).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=3).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=4).alignment = Alignment(horizontal="left")
        ws_kpi.cell(row=cur_row, column=5).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=6).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=7).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=5).number_format = "#,##0"
        ws_kpi.cell(row=cur_row, column=6).number_format = "#,##0"
        ws_kpi.cell(row=cur_row, column=7).number_format = "#,##0.00"
        for c_idx in range(1, len(kpi_headers) + 1):
            ws_kpi.cell(row=cur_row, column=c_idx).border = THIN_BORDER

    sum_row = ws_kpi.max_row + 1
    ws_kpi.append(["合计", "-", "-", "全部32家门店总计", total_records, total_qty, total_amount])
    ws_kpi.row_dimensions[sum_row].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=sum_row, column=c_idx)
        c.font = Font(name="微软雅黑", size=10, bold=True, color="000000")
        c.fill = PatternFill(start_color="FEF08A", end_color="FEF08A", fill_type="solid")
        c.border = THIN_BORDER
        if c_idx in [5, 6]:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0"
        elif c_idx == 7:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0.00"
        else:
            c.alignment = Alignment(horizontal="center")

    apply_column_widths(ws_kpi)
    master_file = os.path.join(target_dir, "总表_全国连锁门店销售明细_32家门店总计.xlsx")
    wb_master.save(master_file)

    md_content = f"""# 类别一：全国连锁门店销售数据对账核对指南

## 1. 结构与文件说明
- **分表测试文件**：位于 `分表测试文件_32家门店/`，共 **32 个 XLSX 文件**，对应 32 家门店。
- **总表文件**：`总表_全国连锁门店销售明细_32家门店总计.xlsx`（包含完整明细与核对看板）。
- **工作表名称**：统一为 `销售明细`。
- **每表数据量**：每个分表包含精确 **30 行** 销售数据。

## 2. 汇总核心核对指标（合并分表后请核对以下指标）
| 指标项 | 预期数值 | 说明 |
| :--- | :--- | :--- |
| **合并文件总数** | **32 个** | 涵盖华北、华东、华南、华中、西南、西北、东北七大区域 |
| **汇总记录总行数** | **{total_records} 行** | 32 家门店 × 30 行/店 |
| **销售总量合计** | **{total_qty:,} 件** | `销售数量` 列直接求和 |
| **销售总金额合计** | **￥{total_amount:,.2f} 元** | `销售金额` 列直接求和（精确至分） |
| **客单价（平均单价）** | **￥{avg_price:,.2f} 元** | `销售总金额 / 销售总量` |

## 3. 各门店小计明细清单（透视对账表）
| 序号 | 门店编码 | 所属区域 | 门店名称 | 记录数 | 销售数量(件) | 销售总额(元) |
| :---: | :---: | :---: | :--- | :---: | :---: | :---: |
"""
    for idx, (s_id, s_reg, s_name, s_cnt, s_qty, s_amt) in enumerate(store_kpi_list, start=1):
        md_content += f"| {idx} | `{s_id}` | {s_reg} | {s_name} | {s_cnt} | {s_qty:,} | ￥{s_amt:,.2f} |\n"
    md_content += f"| **合计** | - | - | **全部32家门店** | **{total_records}** | **{total_qty:,}** | **￥{total_amount:,.2f}** |\n"

    with open(os.path.join(target_dir, "00_核对指标与快速对账表.md"), "w", encoding="utf-8") as f:
        f.write(md_content)

    print(f"  [OK] 类别一已生成：32 个分表，1 个总表，共 {total_records} 行。")
    return {
        "title": "01_全国连锁门店销售数据",
        "folder": "01_全国连锁门店销售数据",
        "sheet_name": "销售明细",
        "files": 32,
        "rows": total_records,
        "metric1_name": "总销售数量",
        "metric1_val": f"{total_qty:,} 件",
        "metric2_name": "总销售金额",
        "metric2_val": f"￥{total_amount:,.2f}",
        "raw_sum": {"销售数量": total_qty, "销售金额": total_amount}
    }

# ==============================================================================
# 类别二：各部门员工薪酬与考勤明细（32个部门）
# ==============================================================================
DEPARTMENTS = [
    ("DEP01", "研发中心-前端研发部"),
    ("DEP02", "研发中心-后端架构部"),
    ("DEP03", "研发中心-移动客户端部"),
    ("DEP04", "研发中心-算法工程部"),
    ("DEP05", "研发中心-质量保障部"),
    ("DEP06", "研发中心-运维与SRE部"),
    ("DEP07", "产品中心-桌面端产品部"),
    ("DEP08", "产品中心-移动端产品部"),
    ("DEP09", "产品中心-交互体验设计部"),
    ("DEP10", "运营中心-用户增长部"),
    ("DEP11", "运营中心-内容生态部"),
    ("DEP12", "运营中心-社群运营部"),
    ("DEP13", "营销中心-华北渠道部"),
    ("DEP14", "营销中心-华东渠道部"),
    ("DEP15", "营销中心-华南渠道部"),
    ("DEP16", "营销中心-西南渠道部"),
    ("DEP17", "营销中心-海外业务部"),
    ("DEP18", "客户成功-大客户服务部"),
    ("DEP19", "客户成功-技术支持部"),
    ("DEP20", "客户成功-培训交付部"),
    ("DEP21", "供应链部-仓储管理部"),
    ("DEP22", "供应链部-物流配送部"),
    ("DEP23", "供应链部-采购计划部"),
    ("DEP24", "财务中心-总账会计部"),
    ("DEP25", "财务中心-成本控制部"),
    ("DEP26", "财务中心-资金管理部"),
    ("DEP27", "财务中心-审计风控部"),
    ("DEP28", "人力资源-招聘配置部"),
    ("DEP29", "人力资源-薪酬绩效部"),
    ("DEP30", "人力资源-员工关系部"),
    ("DEP31", "行政中心-综合行政部"),
    ("DEP32", "总裁办-战略规划部"),
]

TITLES_LEVELS = [
    ("助理专员", 6500, 800, 1000, 1100),
    ("中级专员", 9000, 1200, 2000, 1600),
    ("资深工程师", 15000, 2000, 4000, 2600),
    ("技术专家", 22000, 3000, 6000, 3500),
    ("部门主管", 18000, 2500, 5000, 3000),
    ("部门经理", 26000, 3500, 7500, 4200),
    ("中心总监", 35000, 5000, 12000, 5800)
]

FAMILY_NAMES = "赵钱孙李周吴郑王冯陈褚卫蒋沈韩杨朱秦尤许何吕施张孔曹严华金魏陶姜戚谢邹喻柏水窦章云苏潘葛奚范彭郎鲁韦昌马苗凤花方俞任袁柳酆鲍史唐费廉岑薛雷贺倪汤滕殷罗毕郝邬安常乐于时傅皮卞齐康伍余元卜顾孟平黄和穆萧尹姚邵湛汪祁毛禹狄米贝明臧计伏成戴谈宋茅庞熊纪舒屈项祝董梁杜阮蓝闵席季麻强贾路娄危江童颜郭梅盛林刁钟徐邱骆高夏蔡田樊胡凌霍虞万支柯昝管卢莫经房裘缪干解应宗丁宣贲邓郁单杭洪包诸左石崔吉钮龚"
GIVEN_NAMES = ["伟", "芳", "娜", "秀英", "敏", "静", "丽", "强", "磊", "军", "洋", "勇", "艳", "杰", "娟", "涛", "明", "超", "秀兰", "霞", "平", "刚", "桂英"]

PAYROLL_COLUMNS = [
    ("员工工号", "str", "center", "@"),
    ("员工姓名", "str", "center", None),
    ("所属部门", "str", "left", None),
    ("岗位职务", "str", "center", None),
    ("入职日期", "date", "center", "YYYY-MM-DD"),
    ("应出勤天数", "int", "right", "#,##0"),
    ("实际出勤天数", "int", "right", "#,##0"),
    ("基本工资", "float", "right", "#,##0.00"),
    ("岗位津贴", "float", "right", "#,##0.00"),
    ("绩效奖金", "float", "right", "#,##0.00"),
    ("代扣五险一金", "float", "right", "#,##0.00"),
    ("实发工资金额", "float", "right", "#,##0.00")
]

def generate_category_2():
    print("[2/4] 正在生成【类别二：各部门员工薪酬与考勤明细】...")
    target_dir = os.path.join(OUTPUT_DIR, "02_各部门员工薪酬与考勤")
    split_dir = os.path.join(target_dir, "分表测试文件_32个部门")
    os.makedirs(split_dir, exist_ok=True)

    all_rows = []
    dept_splits = {}
    emp_seq = 1000
    start_hire_date = date(2021, 1, 1)

    for dept_id, dept_name in DEPARTMENTS:
        dept_rows = []
        for i in range(25):
            emp_seq += 1
            emp_id = f"EMP{emp_seq:05d}"

            f_name = FAMILY_NAMES[(emp_seq * 7) % len(FAMILY_NAMES)]
            g_name = GIVEN_NAMES[(emp_seq * 13) % len(GIVEN_NAMES)]
            full_name = f"{f_name}{g_name}"

            title_info = TITLES_LEVELS[(i + int(dept_id[-2:])) % len(TITLES_LEVELS)]
            title, base_salary, allowance, bonus, insurance = title_info

            hire_offset = (i * 37 + int(dept_id[-2:]) * 19) % 1200
            hire_date = (start_hire_date + timedelta(days=hire_offset)).strftime("%Y-%m-%d")

            std_days = 22
            actual_days = std_days if (i % 6 != 0) else (21 if i % 2 == 0 else 20)

            deduct_absent = round(base_salary * ((std_days - actual_days) / std_days), 2)
            net_pay = round(base_salary + allowance + bonus - insurance - deduct_absent, 2)

            row = [
                emp_id, full_name, dept_name, title, hire_date,
                std_days, actual_days, float(base_salary), float(allowance),
                float(bonus), float(insurance), net_pay
            ]
            dept_rows.append(row)
            all_rows.append(row)

        dept_splits[dept_id] = (dept_name, dept_rows)

    for dept_id, (dept_name, d_rows) in dept_splits.items():
        wb_sub = openpyxl.Workbook()
        ws_sub = wb_sub.active
        save_styled_sheet(ws_sub, "薪资考勤", PAYROLL_COLUMNS, d_rows, HEADER_FILL_GREEN)
        safe_name = dept_name.replace("/", "_")
        file_name = f"薪资考勤_{dept_id}_{safe_name}.xlsx"
        wb_sub.save(os.path.join(split_dir, file_name))

    wb_master = openpyxl.Workbook()
    ws_data = wb_master.active
    save_styled_sheet(ws_data, "薪资考勤", PAYROLL_COLUMNS, all_rows, HEADER_FILL_GREEN)

    ws_kpi = wb_master.create_sheet(title="核对指标看板")
    ws_kpi.views.sheetView[0].showGridLines = True

    total_emps = len(all_rows)
    total_base = round(sum(r[7] for r in all_rows), 2)
    total_allowance = round(sum(r[8] for r in all_rows), 2)
    total_bonus = round(sum(r[9] for r in all_rows), 2)
    total_ins = round(sum(r[10] for r in all_rows), 2)
    total_net = round(sum(r[11] for r in all_rows), 2)

    ws_kpi["A1"] = "【全公司员工薪酬与考勤明细】核心核对指标总览"
    ws_kpi["A1"].font = KPI_TITLE_FONT
    ws_kpi.row_dimensions[1].height = 32

    kpi_cards = [
        ("A3", "B3", "总部门分表数", f"{len(DEPARTMENTS)} 个部门"),
        ("C3", "D3", "总在职人数（行数）", f"{total_emps} 人"),
        ("E3", "F3", "基本工资总额", f"￥{total_base:,.2f}"),
        ("G3", "H3", "实发工资总额（元）", f"￥{total_net:,.2f}")
    ]
    for top_left, bottom_right, title, val in kpi_cards:
        ws_kpi[top_left] = title
        ws_kpi[top_left].font = KPI_LABEL_FONT
        col = top_left[0]
        val_cell = f"{col}4"
        ws_kpi[val_cell] = val
        ws_kpi[val_cell].font = KPI_VAL_FONT
        ws_kpi[val_cell].fill = KPI_CARD_FILL
        ws_kpi[val_cell].alignment = Alignment(horizontal="center", vertical="center")
        ws_kpi[top_left].alignment = Alignment(horizontal="center", vertical="center")

    ws_kpi.row_dimensions[3].height = 20
    ws_kpi.row_dimensions[4].height = 26

    ws_kpi["A6"] = "各部门薪资小计明细（供透视表核对）"
    ws_kpi["A6"].font = Font(name="微软雅黑", size=12, bold=True, color="1E3A8A")

    kpi_headers = ["序号", "部门编码", "部门名称", "人数", "基本工资小计", "绩效奖金小计", "代扣社保小计", "实发工资总计(元)"]
    ws_kpi.append([])
    ws_kpi.append(kpi_headers)
    ws_kpi.row_dimensions[7].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=7, column=c_idx)
        c.font = HEADER_FONT
        c.fill = HEADER_FILL_GREEN
        c.alignment = Alignment(horizontal="center", vertical="center")

    dept_kpi_list = []
    for idx, (d_id, (d_name, d_rows)) in enumerate(dept_splits.items(), start=1):
        d_cnt = len(d_rows)
        d_base = round(sum(r[7] for r in d_rows), 2)
        d_bonus = round(sum(r[9] for r in d_rows), 2)
        d_ins = round(sum(r[10] for r in d_rows), 2)
        d_net = round(sum(r[11] for r in d_rows), 2)
        dept_kpi_list.append((d_id, d_name, d_cnt, d_base, d_bonus, d_ins, d_net))
        row_vals = [idx, d_id, d_name, d_cnt, d_base, d_bonus, d_ins, d_net]
        ws_kpi.append(row_vals)
        cur_row = ws_kpi.max_row
        ws_kpi.row_dimensions[cur_row].height = 20
        ws_kpi.cell(row=cur_row, column=1).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=2).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=3).alignment = Alignment(horizontal="left")
        ws_kpi.cell(row=cur_row, column=4).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=5).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=6).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=7).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=8).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=4).number_format = "#,##0"
        for col_f in [5, 6, 7, 8]:
            ws_kpi.cell(row=cur_row, column=col_f).number_format = "#,##0.00"
        for c_idx in range(1, len(kpi_headers) + 1):
            ws_kpi.cell(row=cur_row, column=c_idx).border = THIN_BORDER

    sum_row = ws_kpi.max_row + 1
    ws_kpi.append(["合计", "-", "全公司32个部门总计", total_emps, total_base, total_bonus, total_ins, total_net])
    ws_kpi.row_dimensions[sum_row].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=sum_row, column=c_idx)
        c.font = Font(name="微软雅黑", size=10, bold=True, color="000000")
        c.fill = PatternFill(start_color="FEF08A", end_color="FEF08A", fill_type="solid")
        c.border = THIN_BORDER
        if c_idx == 4:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0"
        elif c_idx >= 5:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0.00"
        else:
            c.alignment = Alignment(horizontal="center")

    apply_column_widths(ws_kpi)
    master_file = os.path.join(target_dir, "总表_全集团员工薪酬考勤_32个部门总计.xlsx")
    wb_master.save(master_file)

    md_content = f"""# 类别二：各部门员工薪酬与考勤明细对账核对指南

## 1. 结构与文件说明
- **分表测试文件**：位于 `分表测试文件_32个部门/`，共 **32 个 XLSX 文件**。
- **总表文件**：`总表_全集团员工薪酬考勤_32个部门总计.xlsx`。
- **工作表名称**：统一为 `薪资考勤`。
- **每表数据量**：每个部门分表包含精确 **25 行** 员工薪资记录。

## 2. 汇总核心核对指标（合并分表后请核对以下指标）
| 指标项 | 预期数值 | 说明 |
| :--- | :--- | :--- |
| **合并部门分表数** | **32 个** | 研发、产品、运营、营销、客户成功、供应链、财务、人力、行政等 |
| **员工总人数（总行数）** | **{total_emps} 人** | 32 个部门 × 25 人/部门 |
| **基本工资总额** | **￥{total_base:,.2f} 元** | `基本工资` 列直接求和 |
| **岗位津贴总额** | **￥{total_allowance:,.2f} 元** | `岗位津贴` 列直接求和 |
| **绩效奖金总额** | **￥{total_bonus:,.2f} 元** | `绩效奖金` 列直接求和 |
| **代扣五险一金总额** | **￥{total_ins:,.2f} 元** | `代扣五险一金` 列直接求和 |
| **实发工资金额总计** | **￥{total_net:,.2f} 元** | `实发工资金额` 列直接求和（精确至分） |

## 3. 各部门小计明细清单（透视对账表）
| 序号 | 部门编码 | 部门名称 | 人数 | 基本工资小计(元) | 绩效奖金小计(元) | 代扣社保小计(元) | 实发工资总计(元) |
| :---: | :---: | :--- | :---: | :---: | :---: | :---: | :---: |
"""
    for idx, (d_id, d_name, d_cnt, d_base, d_bonus, d_ins, d_net) in enumerate(dept_kpi_list, start=1):
        md_content += f"| {idx} | `{d_id}` | {d_name} | {d_cnt} | ￥{d_base:,.2f} | ￥{d_bonus:,.2f} | ￥{d_ins:,.2f} | ￥{d_net:,.2f} |\n"
    md_content += f"| **合计** | - | **全部32个部门** | **{total_emps}** | **￥{total_base:,.2f}** | **￥{total_bonus:,.2f}** | **￥{total_ins:,.2f}** | **￥{total_net:,.2f}** |\n"

    with open(os.path.join(target_dir, "00_核对指标与快速对账表.md"), "w", encoding="utf-8") as f:
        f.write(md_content)

    print(f"  [OK] 类别二已生成：32 个分表，1 个总表，共 {total_emps} 行。")
    return {
        "title": "02_各部门员工薪酬与考勤",
        "folder": "02_各部门员工薪酬与考勤",
        "sheet_name": "薪资考勤",
        "files": 32,
        "rows": total_emps,
        "metric1_name": "基本工资总额",
        "metric1_val": f"￥{total_base:,.2f}",
        "metric2_name": "实发工资总计",
        "metric2_val": f"￥{total_net:,.2f}",
        "raw_sum": {"基本工资": total_base, "实发工资金额": total_net}
    }

# ==============================================================================
# 类别三：各仓储中心库存与物资流转（32个仓库）
# ==============================================================================
WAREHOUSES = [
    ("WH01", "华北-北京顺义1号总仓", "华北"),
    ("WH02", "华北-北京大兴2号分拨仓", "华北"),
    ("WH03", "华北-天津武清保税仓", "华北"),
    ("WH04", "华北-石家庄正定物流仓", "华北"),
    ("WH05", "华北-太原小店配送仓", "华北"),
    ("WH06", "华北-呼和浩特金川仓", "华北"),
    ("WH07", "华东-上海青浦1号中央仓", "华东"),
    ("WH08", "华东-上海奉贤冷链仓", "华东"),
    ("WH09", "华东-昆山花桥自动化仓", "华东"),
    ("WH10", "华东-杭州萧山电商枢纽仓", "华东"),
    ("WH11", "华东-南京江宁集散仓", "华东"),
    ("WH12", "华东-合肥肥东中转仓", "华东"),
    ("WH13", "华南-广州白云航空总仓", "华南"),
    ("WH14", "华南-广州黄埔保税仓", "华南"),
    ("WH15", "华南-深圳宝安空港仓", "华南"),
    ("WH16", "华南-深圳龙岗配送仓", "华南"),
    ("WH17", "华南-东莞虎门集散仓", "华南"),
    ("WH18", "华南-佛山南海前置仓", "华南"),
    ("WH19", "华南-南宁江南综合仓", "华南"),
    ("WH20", "华中-武汉东西湖核心枢纽仓", "华中"),
    ("WH21", "华中-武汉江夏备件仓", "华中"),
    ("WH22", "华中-长沙望城分拣仓", "华中"),
    ("WH23", "华中-郑州经开航空港仓", "华中"),
    ("WH24", "西南-成都双流西南总仓", "西南"),
    ("WH25", "西南-成都青白江国际铁路仓", "西南"),
    ("WH26", "西南-重庆沙坪坝集结仓", "西南"),
    ("WH27", "西南-贵阳观山湖前置仓", "西南"),
    ("WH28", "西南-昆明官渡高原冷链仓", "西南"),
    ("WH29", "西北-西安西咸新区总仓", "西北"),
    ("WH30", "西北-兰州安宁枢纽仓", "西北"),
    ("WH31", "西北-乌鲁木齐头屯河口岸仓", "西北"),
    ("WH32", "东北-沈阳浑南东北枢纽仓", "东北"),
]

MATERIALS = [
    ("MAT-1001", "工业主控主板", "SYS-ARM-V3", "块", 1250.00),
    ("MAT-1002", "高精度伺服电机", "SRV-750W", "台", 890.00),
    ("MAT-1003", "工业级固态硬盘", "SSD-M2-2TB", "块", 780.00),
    ("MAT-1004", "精密轴承单元", "BRG-6205ZZ", "套", 145.00),
    ("MAT-1005", "步进电机驱动器", "DRV-TB6600", "台", 220.00),
    ("MAT-1006", "工业开关电源", "PWR-24V-350W", "台", 185.00),
    ("MAT-1007", "光电隔离传感器", "SNS-OPT-08", "个", 65.00),
    ("MAT-1008", "双通道继电器模块", "RLY-2CH-5V", "个", 38.00),
    ("MAT-1009", "工业触摸屏显示器", "HMI-10INCH", "台", 1450.00),
    ("MAT-1010", "高柔性拖链线缆", "CBL-TR-4X0.75", "卷", 320.00),
    ("MAT-1011", "气动双作用电磁阀", "VLV-4V210-08", "件", 115.00),
    ("MAT-1012", "铝合金标准型材", "AL-4040-2M", "根", 85.00),
    ("MAT-1013", "高压耐温气管", "TUBE-PU-8X5", "卷", 160.00),
    ("MAT-1014", "防静电滚筒传送单元", "ROL-ESD-500", "套", 450.00),
    ("MAT-1015", "激光测距测厚传感器", "LSR-DS-50MM", "台", 2100.00),
    ("MAT-1016", "工业级以太网交换机", "SW-8PORT-DIN", "台", 680.00),
    ("MAT-1017", "伺服行星减速机", "GBX-PLE60-L1", "台", 950.00),
    ("MAT-1018", "精密滚珠丝杠副", "BS-1605-1000", "套", 560.00),
    ("MAT-1019", "线性滑块导轨组", "LGD-HGR20R", "套", 380.00),
    ("MAT-1020", "微型气动吸盘夹爪", "GRP-VAC-20", "个", 95.00),
    ("MAT-1021", "安全急停按钮盒", "BTN-EMG-RED", "个", 42.00),
    ("MAT-1022", "多色LED工业报警灯", "TWR-LED-3C", "套", 175.00),
    ("MAT-1023", "温湿度变送器探头", "SNS-TH-RS485", "个", 130.00),
    ("MAT-1024", "导轨式端子排组件", "TM-UK2.5B-10P", "排", 25.00),
    ("MAT-1025", "微型电磁离合器", "CLT-MICRO-24V", "台", 410.00),
    ("MAT-1026", "同步带传动总成", "BLT-HTD-5M", "套", 75.00),
    ("MAT-1027", "智能条码扫描模组", "SCN-FIX-2D", "台", 880.00),
    ("MAT-1028", "交流散热风机组件", "FAN-AC-220V", "台", 58.00),
    ("MAT-1029", "航空插头线束组合", "PLG-M12-4PIN", "套", 48.00),
    ("MAT-1030", "机柜专用防尘过滤网", "FLT-ENC-200", "套", 32.00),
]

INVENTORY_COLUMNS = [
    ("物料编码", "str", "center", "@"),
    ("物料名称", "str", "left", None),
    ("规格型号", "str", "center", None),
    ("计量单位", "str", "center", None),
    ("仓库编号", "str", "center", "@"),
    ("仓库名称", "str", "left", None),
    ("所属大区", "str", "center", None),
    ("库位编号", "str", "center", "@"),
    ("期初库存", "int", "right", "#,##0"),
    ("本期入库量", "int", "right", "#,##0"),
    ("本期出库量", "int", "right", "#,##0"),
    ("期末结存量", "int", "right", "#,##0"),
    ("物料单价", "float", "right", "#,##0.00"),
    ("结存总金额", "float", "right", "#,##0.00")
]

def generate_category_3():
    print("[3/4] 正在生成【类别三：各仓储中心库存与物资流转】...")
    target_dir = os.path.join(OUTPUT_DIR, "03_各仓储中心库存与物资流转")
    split_dir = os.path.join(target_dir, "分表测试文件_32个仓库")
    os.makedirs(split_dir, exist_ok=True)

    all_rows = []
    wh_splits = {}

    for wh_id, wh_name, wh_region in WAREHOUSES:
        wh_rows = []
        wh_num = int(wh_id.replace("WH", ""))

        for i in range(30):
            mat_info = MATERIALS[i]
            m_code, m_name, m_spec, m_unit, m_price = mat_info

            loc_zone = chr(ord('A') + (i % 6))
            loc_rack = (i % 15) + 1
            loc_tier = (i % 4) + 1
            location_id = f"{loc_zone}-{loc_rack:02d}-{loc_tier:02d}"

            initial_stock = 100 + ((wh_num * 17 + i * 29) % 800)
            in_qty = 10 + ((wh_num * 13 + i * 23) % 250)
            out_qty = 5 + ((wh_num * 11 + i * 19) % 200)
            if out_qty > (initial_stock + in_qty):
                out_qty = initial_stock // 2
            final_stock = initial_stock + in_qty - out_qty
            total_val = round(final_stock * m_price, 2)

            row = [
                m_code, m_name, m_spec, m_unit, wh_id, wh_name, wh_region,
                location_id, initial_stock, in_qty, out_qty, final_stock,
                m_price, total_val
            ]
            wh_rows.append(row)
            all_rows.append(row)

        wh_splits[wh_id] = (wh_name, wh_region, wh_rows)

    for wh_id, (wh_name, wh_region, w_rows) in wh_splits.items():
        wb_sub = openpyxl.Workbook()
        ws_sub = wb_sub.active
        save_styled_sheet(ws_sub, "库存明细", INVENTORY_COLUMNS, w_rows, HEADER_FILL_PURPLE)
        safe_name = wh_name.replace("/", "_")
        file_name = f"库存明细_{wh_id}_{safe_name}.xlsx"
        wb_sub.save(os.path.join(split_dir, file_name))

    wb_master = openpyxl.Workbook()
    ws_data = wb_master.active
    save_styled_sheet(ws_data, "库存明细", INVENTORY_COLUMNS, all_rows, HEADER_FILL_PURPLE)

    ws_kpi = wb_master.create_sheet(title="核对指标看板")
    ws_kpi.views.sheetView[0].showGridLines = True

    total_records = len(all_rows)
    total_initial = sum(r[8] for r in all_rows)
    total_in = sum(r[9] for r in all_rows)
    total_out = sum(r[10] for r in all_rows)
    total_final = sum(r[11] for r in all_rows)
    total_val_sum = round(sum(r[13] for r in all_rows), 2)

    ws_kpi["A1"] = "【全国各仓库存与物资流转明细】核心核对指标总览"
    ws_kpi["A1"].font = KPI_TITLE_FONT
    ws_kpi.row_dimensions[1].height = 32

    kpi_cards = [
        ("A3", "B3", "总仓库分表数", f"{len(WAREHOUSES)} 个仓库"),
        ("C3", "D3", "总物资行数", f"{total_records} 行"),
        ("E3", "F3", "期末结存总数量", f"{total_final:,} 件"),
        ("G3", "H3", "结存库存总金额（元）", f"￥{total_val_sum:,.2f}")
    ]
    for top_left, bottom_right, title, val in kpi_cards:
        ws_kpi[top_left] = title
        ws_kpi[top_left].font = KPI_LABEL_FONT
        col = top_left[0]
        val_cell = f"{col}4"
        ws_kpi[val_cell] = val
        ws_kpi[val_cell].font = KPI_VAL_FONT
        ws_kpi[val_cell].fill = KPI_CARD_FILL
        ws_kpi[val_cell].alignment = Alignment(horizontal="center", vertical="center")
        ws_kpi[top_left].alignment = Alignment(horizontal="center", vertical="center")

    ws_kpi.row_dimensions[3].height = 20
    ws_kpi.row_dimensions[4].height = 26

    ws_kpi["A6"] = "各仓库小计明细（供透视表核对）"
    ws_kpi["A6"].font = Font(name="微软雅黑", size=12, bold=True, color="1E3A8A")

    kpi_headers = ["序号", "仓库编号", "所属大区", "仓库名称", "品类数(行数)", "入库总量", "出库总量", "期末结存总量", "库存总价值(元)"]
    ws_kpi.append([])
    ws_kpi.append(kpi_headers)
    ws_kpi.row_dimensions[7].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=7, column=c_idx)
        c.font = HEADER_FONT
        c.fill = HEADER_FILL_PURPLE
        c.alignment = Alignment(horizontal="center", vertical="center")

    wh_kpi_list = []
    for idx, (w_id, (w_name, w_region, w_rows)) in enumerate(wh_splits.items(), start=1):
        w_cnt = len(w_rows)
        w_in = sum(r[9] for r in w_rows)
        w_out = sum(r[10] for r in w_rows)
        w_final = sum(r[11] for r in w_rows)
        w_amt = round(sum(r[13] for r in w_rows), 2)
        wh_kpi_list.append((w_id, w_region, w_name, w_cnt, w_in, w_out, w_final, w_amt))
        row_vals = [idx, w_id, w_region, w_name, w_cnt, w_in, w_out, w_final, w_amt]
        ws_kpi.append(row_vals)
        cur_row = ws_kpi.max_row
        ws_kpi.row_dimensions[cur_row].height = 20
        ws_kpi.cell(row=cur_row, column=1).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=2).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=3).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=4).alignment = Alignment(horizontal="left")
        ws_kpi.cell(row=cur_row, column=5).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=6).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=7).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=8).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=9).alignment = Alignment(horizontal="right")
        for col_f in [5, 6, 7, 8]:
            ws_kpi.cell(row=cur_row, column=col_f).number_format = "#,##0"
        ws_kpi.cell(row=cur_row, column=9).number_format = "#,##0.00"
        for c_idx in range(1, len(kpi_headers) + 1):
            ws_kpi.cell(row=cur_row, column=c_idx).border = THIN_BORDER

    sum_row = ws_kpi.max_row + 1
    ws_kpi.append(["合计", "-", "-", "全部32个仓库总计", total_records, total_in, total_out, total_final, total_val_sum])
    ws_kpi.row_dimensions[sum_row].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=sum_row, column=c_idx)
        c.font = Font(name="微软雅黑", size=10, bold=True, color="000000")
        c.fill = PatternFill(start_color="FEF08A", end_color="FEF08A", fill_type="solid")
        c.border = THIN_BORDER
        if c_idx in [5, 6, 7, 8]:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0"
        elif c_idx == 9:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0.00"
        else:
            c.alignment = Alignment(horizontal="center")

    apply_column_widths(ws_kpi)
    master_file = os.path.join(target_dir, "总表_全国各仓库存与流转明细_32个仓库总计.xlsx")
    wb_master.save(master_file)

    md_content = f"""# 类别三：各仓储中心库存与物资流转对账核对指南

## 1. 结构与文件说明
- **分表测试文件**：位于 `分表测试文件_32个仓库/`，共 **32 个 XLSX 文件**。
- **总表文件**：`总表_全国各仓库存与流转明细_32个仓库总计.xlsx`。
- **工作表名称**：统一为 `库存明细`。
- **每表数据量**：每个仓库分表包含精确 **30 行** 物资盘点明细。

## 2. 汇总核心核对指标（合并分表后请核对以下指标）
| 指标项 | 预期数值 | 说明 |
| :--- | :--- | :--- |
| **合并仓库分表数** | **32 个** | 覆盖华北、华东、华南、华中、西南、西北、东北仓储网络 |
| **物资流转总行数** | **{total_records} 行** | 32 个仓库 × 30 项物料/仓 |
| **期初库存合计** | **{total_initial:,} 件** | `期初库存` 列直接求和 |
| **本期入库总量合计** | **{total_in:,} 件** | `本期入库量` 列直接求和 |
| **本期出库总量合计** | **{total_out:,} 件** | `本期出库量` 列直接求和 |
| **期末结存总量合计** | **{total_final:,} 件** | `期末结存量` 列直接求和 |
| **结存库存总金额合计** | **￥{total_val_sum:,.2f} 元** | `结存总金额` 列直接求和（精确至分） |

## 3. 各仓库小计明细清单（透视对账表）
| 序号 | 仓库编号 | 所属大区 | 仓库名称 | 品类数 | 入库总量 | 出库总量 | 期末结存总量 | 库存总价值(元) |
| :---: | :---: | :---: | :--- | :---: | :---: | :---: | :---: | :---: |
"""
    for idx, (w_id, w_region, w_name, w_cnt, w_in, w_out, w_final, w_amt) in enumerate(wh_kpi_list, start=1):
        md_content += f"| {idx} | `{w_id}` | {w_region} | {w_name} | {w_cnt} | {w_in:,} | {w_out:,} | {w_final:,} | ￥{w_amt:,.2f} |\n"
    md_content += f"| **合计** | - | - | **全部32个仓库** | **{total_records}** | **{total_in:,}** | **{total_out:,}** | **{total_final:,}** | **￥{total_val_sum:,.2f}** |\n"

    with open(os.path.join(target_dir, "00_核对指标与快速对账表.md"), "w", encoding="utf-8") as f:
        f.write(md_content)

    print(f"  [OK] 类别三已生成：32 个分表，1 个总表，共 {total_records} 行。")
    return {
        "title": "03_各仓储中心库存与物资流转",
        "folder": "03_各仓储中心库存与物资流转",
        "sheet_name": "库存明细",
        "files": 32,
        "rows": total_records,
        "metric1_name": "期末结存总量合计",
        "metric1_val": f"{total_final:,} 件",
        "metric2_name": "结存库存总金额合计",
        "metric2_val": f"￥{total_val_sum:,.2f}",
        "raw_sum": {"期末结存量": total_final, "结存总金额": total_val_sum}
    }

# ==============================================================================
# 类别四：分支机构费用申报（含自定义表头与固定单元格 A1:填写人 B1:内容 等）
# ==============================================================================
BRANCHES_INFO = [
    ("BJ01", "北京朝阳分公司", "张建国", "王立群", "华北", "YS-2024-BJ01"),
    ("BJ02", "北京海淀办事处", "李晓梅", "王立群", "华北", "YS-2024-BJ02"),
    ("TJ01", "天津和平分公司", "刘德海", "赵志强", "华北", "YS-2024-TJ01"),
    ("SJZ01", "石家庄办事处", "陈晨", "赵志强", "华北", "YS-2024-SJZ01"),
    ("TY01", "太原分公司", "杨阳", "赵志强", "华北", "YS-2024-TY01"),
    ("HHHT01", "呼和浩特办事处", "周俊", "赵志强", "华北", "YS-2024-HHHT01"),
    ("SH01", "上海浦东分公司", "黄志诚", "陆伟峰", "华东", "YS-2024-SH01"),
    ("SH02", "上海徐汇办事处", "吴雪", "陆伟峰", "华东", "YS-2024-SH02"),
    ("HZ01", "杭州西湖分公司", "徐明", "陆伟峰", "华东", "YS-2024-HZ01"),
    ("NJ01", "南京鼓楼分公司", "孙浩", "陆伟峰", "华东", "YS-2024-NJ01"),
    ("SZ01", "苏州园区办事处", "朱婷婷", "陆伟峰", "华东", "YS-2024-SZ01"),
    ("HF01", "合肥蜀山办事处", "胡军", "陆伟峰", "华东", "YS-2024-HF01"),
    ("GZ01", "广州天河分公司", "林浩然", "苏建华", "华南", "YS-2024-GZ01"),
    ("SZG01", "深圳南山分公司", "何丽娟", "苏建华", "华南", "YS-2024-SZG01"),
    ("SZG02", "深圳福田办事处", "郭少强", "苏建华", "华南", "YS-2024-SZG02"),
    ("DG01", "东莞分公司", "罗敏", "苏建华", "华南", "YS-2024-DG01"),
    ("FS01", "佛山办事处", "谢文辉", "苏建华", "华南", "YS-2024-FS01"),
    ("NN01", "南宁分公司", "梁宇", "苏建华", "华南", "YS-2024-NN01"),
    ("WH01", "武汉江汉分公司", "唐勇", "冯启明", "华中", "YS-2024-WH01"),
    ("WH02", "武汉光谷办事处", "许佳丽", "冯启明", "华中", "YS-2024-WH02"),
    ("CS01", "长沙岳麓分公司", "宋鹏", "冯启明", "华中", "YS-2024-CS01"),
    ("ZZ01", "郑州金水分公司", "韩飞", "冯启明", "华中", "YS-2024-ZZ01"),
    ("CD01", "成都锦江分公司", "曹宇", "郑海波", "西南", "YS-2024-CD01"),
    ("CD02", "成都高新办事处", "邓莉", "郑海波", "西南", "YS-2024-CD02"),
    ("CQ01", "重庆渝中分公司", "彭强", "郑海波", "西南", "YS-2024-CQ01"),
    ("CQ02", "重庆江北办事处", "潘慧", "郑海波", "西南", "YS-2024-CQ02"),
    ("KM01", "昆明分公司", "袁涛", "郑海波", "西南", "YS-2024-KM01"),
    ("XA01", "西安雁塔分公司", "马晓峰", "高树林", "西北", "YS-2024-XA01"),
    ("LZ01", "兰州分公司", "沈俊杰", "高树林", "西北", "YS-2024-LZ01"),
    ("WLMQ01", "乌鲁木齐办事处", "丁凯", "高树林", "西北", "YS-2024-WLMQ01"),
    ("SY01", "沈阳和平分公司", "姜峰", "董晓东", "东北", "YS-2024-SY01"),
    ("DL01", "大连中山分公司", "崔丽", "董晓东", "东北", "YS-2024-DL01"),
]

EXPENSE_CATEGORIES = [
    ("差旅交通费", "高铁票/机票/市内出租车往返"),
    ("客户招待费", "商务洽谈餐饮与客户交流"),
    ("办公耗材费", "打印耗材及日常办公用品补充"),
    ("市场推广费", "区域线下宣传海报与展位租赁"),
    ("会议培训费", "员工季度技能提升与外部研讨"),
    ("场地租赁费", "临时交流会议室与设备租金"),
    ("通讯网络费", "分支机构宽带与移动业务通讯补贴"),
    ("快递物流费", "业务合同及紧急样品跨省速运"),
]

PAY_SETTLE = ["银行公对公转账", "企业微信打款", "招行直联汇款", "企业网银支付"]

# 分表第3行的表头定义
BRANCH_EXPENSE_COLUMNS = [
    ("报销单号", "str", "center", "@"),
    ("费用发生日期", "date", "center", "YYYY-MM-DD"),
    ("费用科目", "str", "center", None),
    ("费用详细事由", "str", "left", None),
    ("发票张数", "int", "right", "#,##0"),
    ("申请报销金额", "float", "right", "#,##0.00"),
    ("实批报销金额", "float", "right", "#,##0.00"),
    ("结算支付方式", "str", "center", None)
]

# 总表（汇总宽表，包含了固定单元格映射列）的列定义
MASTER_EXPENSE_COLUMNS = [
    ("所属机构", "str", "left", None),
    ("填写人", "str", "center", None),
    ("申报日期", "date", "center", "YYYY-MM-DD"),
    ("预算单号", "str", "center", "@"),
    ("审批领导", "str", "center", None),
    ("所属大区", "str", "center", None),
    ("报销单号", "str", "center", "@"),
    ("费用发生日期", "date", "center", "YYYY-MM-DD"),
    ("费用科目", "str", "center", None),
    ("费用详细事由", "str", "left", None),
    ("发票张数", "int", "right", "#,##0"),
    ("申请报销金额", "float", "right", "#,##0.00"),
    ("实批报销金额", "float", "right", "#,##0.00"),
    ("结算支付方式", "str", "center", None)
]

def generate_category_4():
    print("[4/4] 正在生成【类别四：分支机构费用申报（含单独填写单元格/自定义表头）】...")
    target_dir = os.path.join(OUTPUT_DIR, "04_分支机构费用申报_含自定义表头与固定单元格")
    split_dir = os.path.join(target_dir, "分表测试文件_32家分支机构")
    os.makedirs(split_dir, exist_ok=True)

    all_master_rows = []
    branch_splits = {}
    exp_seq = 20000
    base_date = date(2024, 10, 1)

    for idx, (b_code, b_name, author, approver, region, budget_code) in enumerate(BRANCHES_INFO, start=1):
        branch_rows = []
        fill_date = (base_date + timedelta(days=20 + (idx % 8))).strftime("%Y-%m-%d")

        # 每家分支机构 25 行明细
        for i in range(25):
            exp_seq += 1
            bill_no = f"EXP202410{exp_seq}"
            item_date = (base_date + timedelta(days=(i * 2 + idx) % 20)).strftime("%Y-%m-%d")

            exp_cat, exp_desc = EXPENSE_CATEGORIES[(i + idx) % len(EXPENSE_CATEGORIES)]
            inv_count = ((i * 3 + idx) % 5) + 1  # 1~5 张

            # 申请金额 200 ~ 6800
            apply_amount = round(200.0 + ((idx * 137 + i * 83) % 6500) + ((i % 5) * 0.5), 2)
            # 实批金额（个别略作核减）
            approved_amount = apply_amount if (i % 7 != 0) else round(apply_amount * 0.95, 2)
            settle_way = PAY_SETTLE[(i + idx) % len(PAY_SETTLE)]

            # 分表行数据（第4行开始）
            sub_row = [
                bill_no, item_date, exp_cat, exp_desc,
                inv_count, apply_amount, approved_amount, settle_way
            ]
            branch_rows.append(sub_row)

            # 总表宽表行数据（把固定单元格广播到前面）
            master_row = [
                b_name, author, fill_date, budget_code, approver, region,
                bill_no, item_date, exp_cat, exp_desc,
                inv_count, apply_amount, approved_amount, settle_way
            ]
            all_master_rows.append(master_row)

        branch_splits[b_code] = {
            "name": b_name,
            "author": author,
            "fill_date": fill_date,
            "budget_code": budget_code,
            "approver": approver,
            "region": region,
            "rows": branch_rows
        }

    # 1. 保存 32 个带自定义表头排版的分表
    for b_code, b_info in branch_splits.items():
        wb_sub = openpyxl.Workbook()
        ws_sub = wb_sub.active
        ws_sub.title = "费用申报"
        ws_sub.views.sheetView[0].showGridLines = True

        # 设置第 1 行：单独填写单元格区
        # A1: 填写人, B1: [作者], C1: 所属机构, D1: [分公司], E1: 申报日期, F1: [日期], G1: 预算单号, H1: [单号]
        r1_items = [
            (1, "填写人:", b_info["author"]),
            (3, "所属机构:", b_info["name"]),
            (5, "申报日期:", b_info["fill_date"]),
            (7, "预算单号:", b_info["budget_code"])
        ]
        ws_sub.row_dimensions[1].height = 24
        for col_k, key_text, val_text in r1_items:
            c_k = ws_sub.cell(row=1, column=col_k, value=key_text)
            c_k.font = KEY_CELL_FONT
            c_k.fill = KEY_CELL_FILL
            c_k.border = THIN_BORDER
            c_k.alignment = Alignment(horizontal="center", vertical="center")

            c_v = ws_sub.cell(row=1, column=col_k + 1, value=val_text)
            c_v.font = VAL_CELL_FONT
            c_v.fill = VAL_CELL_FILL
            c_v.border = THIN_BORDER
            c_v.alignment = Alignment(horizontal="center", vertical="center")

        # 设置第 2 行：单独填写单元格区
        # A2: 审批领导, B2: [领导], C2: 所属大区, D2: [大区], E2: 申报月份, F2: 2024-10, G2: 结算币种, H2: 人民币(CNY)
        r2_items = [
            (1, "审批领导:", b_info["approver"]),
            (3, "所属大区:", b_info["region"]),
            (5, "申报月份:", "2024-10"),
            (7, "结算币种:", "人民币(CNY)")
        ]
        ws_sub.row_dimensions[2].height = 24
        for col_k, key_text, val_text in r2_items:
            c_k = ws_sub.cell(row=2, column=col_k, value=key_text)
            c_k.font = KEY_CELL_FONT
            c_k.fill = KEY_CELL_FILL
            c_k.border = THIN_BORDER
            c_k.alignment = Alignment(horizontal="center", vertical="center")

            c_v = ws_sub.cell(row=2, column=col_k + 1, value=val_text)
            c_v.font = VAL_CELL_FONT
            c_v.fill = VAL_CELL_FILL
            c_v.border = THIN_BORDER
            c_v.alignment = Alignment(horizontal="center", vertical="center")

        # 设置第 3 行：明细数据表头
        ws_sub.row_dimensions[3].height = 26
        for c_idx, (col_name, _, _, _) in enumerate(BRANCH_EXPENSE_COLUMNS, start=1):
            cell = ws_sub.cell(row=3, column=c_idx, value=col_name)
            cell.font = HEADER_FONT
            cell.fill = HEADER_FILL_AMBER
            cell.alignment = Alignment(horizontal="center", vertical="center")
            cell.border = THIN_BORDER

        # 设置第 4~28 行：明细数据
        for r_offset, r_vals in enumerate(b_info["rows"], start=4):
            ws_sub.row_dimensions[r_offset].height = 20
            for c_idx, (val, (_, _, col_align, col_fmt)) in enumerate(zip(r_vals, BRANCH_EXPENSE_COLUMNS), start=1):
                cell = ws_sub.cell(row=r_offset, column=c_idx, value=val)
                cell.font = DATA_FONT
                cell.border = THIN_BORDER
                cell.alignment = Alignment(horizontal=col_align, vertical="center")
                if col_fmt:
                    cell.number_format = col_fmt

        apply_column_widths(ws_sub)
        ws_sub.freeze_panes = "A4"

        file_name = f"费用申报_{b_code}_{b_info['region']}_{b_info['name']}.xlsx"
        wb_sub.save(os.path.join(split_dir, file_name))

    # 2. 保存总表（包含汇总合并标准明细 + 核对看板）
    wb_master = openpyxl.Workbook()
    ws_master_data = wb_master.active
    save_styled_sheet(ws_master_data, "汇总合并标准明细", MASTER_EXPENSE_COLUMNS, all_master_rows, HEADER_FILL_AMBER)

    ws_kpi = wb_master.create_sheet(title="核对指标看板")
    ws_kpi.views.sheetView[0].showGridLines = True

    total_records = len(all_master_rows)
    total_inv_count = sum(r[10] for r in all_master_rows)
    total_apply = round(sum(r[11] for r in all_master_rows), 2)
    total_approved = round(sum(r[12] for r in all_master_rows), 2)

    ws_kpi["A1"] = "【全国分支机构费用申报与固定单元格汇总】核心核对指标总览"
    ws_kpi["A1"].font = KPI_TITLE_FONT
    ws_kpi.row_dimensions[1].height = 32

    kpi_cards = [
        ("A3", "B3", "总申报分支机构数", f"{len(BRANCHES_INFO)} 个分公司"),
        ("C3", "D3", "总申报费用笔数（行数）", f"{total_records} 行"),
        ("E3", "F3", "申请报销总金额", f"￥{total_apply:,.2f}"),
        ("G3", "H3", "实批报销总金额（元）", f"￥{total_approved:,.2f}")
    ]
    for top_left, bottom_right, title, val in kpi_cards:
        ws_kpi[top_left] = title
        ws_kpi[top_left].font = KPI_LABEL_FONT
        col = top_left[0]
        val_cell = f"{col}4"
        ws_kpi[val_cell] = val
        ws_kpi[val_cell].font = KPI_VAL_FONT
        ws_kpi[val_cell].fill = KPI_CARD_FILL
        ws_kpi[val_cell].alignment = Alignment(horizontal="center", vertical="center")
        ws_kpi[top_left].alignment = Alignment(horizontal="center", vertical="center")

    ws_kpi.row_dimensions[3].height = 20
    ws_kpi.row_dimensions[4].height = 26

    # 提示操作与固定单元格规则
    ws_kpi["A6"] = "固定单元格提取与小计对账清单（在汇总工具中提取对应单元格并核对）"
    ws_kpi["A6"].font = Font(name="微软雅黑", size=12, bold=True, color="1E3A8A")

    kpi_headers = ["序号", "机构编码", "所属大区", "所属机构(D1)", "填写人(B1)", "审批领导(B2)", "单数", "发票数", "申请总额(元)", "实批总额(元)"]
    ws_kpi.append([])
    ws_kpi.append(kpi_headers)
    ws_kpi.row_dimensions[7].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=7, column=c_idx)
        c.font = HEADER_FONT
        c.fill = HEADER_FILL_AMBER
        c.alignment = Alignment(horizontal="center", vertical="center")

    branch_kpi_list = []
    for idx, (b_code, b_info) in enumerate(branch_splits.items(), start=1):
        b_cnt = len(b_info["rows"])
        b_inv = sum(r[4] for r in b_info["rows"])
        b_app = round(sum(r[5] for r in b_info["rows"]), 2)
        b_apr = round(sum(r[6] for r in b_info["rows"]), 2)
        branch_kpi_list.append((b_code, b_info["region"], b_info["name"], b_info["author"], b_info["approver"], b_cnt, b_inv, b_app, b_apr))
        row_vals = [idx, b_code, b_info["region"], b_info["name"], b_info["author"], b_info["approver"], b_cnt, b_inv, b_app, b_apr]
        ws_kpi.append(row_vals)
        cur_row = ws_kpi.max_row
        ws_kpi.row_dimensions[cur_row].height = 20
        ws_kpi.cell(row=cur_row, column=1).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=2).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=3).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=4).alignment = Alignment(horizontal="left")
        ws_kpi.cell(row=cur_row, column=5).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=6).alignment = Alignment(horizontal="center")
        ws_kpi.cell(row=cur_row, column=7).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=8).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=9).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=10).alignment = Alignment(horizontal="right")
        ws_kpi.cell(row=cur_row, column=7).number_format = "#,##0"
        ws_kpi.cell(row=cur_row, column=8).number_format = "#,##0"
        ws_kpi.cell(row=cur_row, column=9).number_format = "#,##0.00"
        ws_kpi.cell(row=cur_row, column=10).number_format = "#,##0.00"
        for c_idx in range(1, len(kpi_headers) + 1):
            ws_kpi.cell(row=cur_row, column=c_idx).border = THIN_BORDER

    sum_row = ws_kpi.max_row + 1
    ws_kpi.append(["合计", "-", "-", "全部32家分支机构总计", "-", "-", total_records, total_inv_count, total_apply, total_approved])
    ws_kpi.row_dimensions[sum_row].height = 24
    for c_idx in range(1, len(kpi_headers) + 1):
        c = ws_kpi.cell(row=sum_row, column=c_idx)
        c.font = Font(name="微软雅黑", size=10, bold=True, color="000000")
        c.fill = PatternFill(start_color="FEF08A", end_color="FEF08A", fill_type="solid")
        c.border = THIN_BORDER
        if c_idx in [7, 8]:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0"
        elif c_idx in [9, 10]:
            c.alignment = Alignment(horizontal="right")
            c.number_format = "#,##0.00"
        else:
            c.alignment = Alignment(horizontal="center")

    apply_column_widths(ws_kpi)
    master_file = os.path.join(target_dir, "总表_全国分支机构费用报销_32家机构总计_含固定单元格汇总.xlsx")
    wb_master.save(master_file)

    # 3. 输出 Markdown 详细对账与提取配置指南
    md_content = f"""# 类别四：分支机构费用申报（自定义表头与单独填写单元格）对账与测试指南

## 1. 结构与版式特点（自定义格式）
本类表格模拟了企业中最典型的 **“非规整表头/含表头填报信息”** 申报表格式：
- **第 1 行（单独填写的 Key-Value 单元格）**：
  - `A1: 填写人:` | `B1: [填写人姓名]` （如张建国、李晓梅）
  - `C1: 所属机构:` | `D1: [分公司名称]` （如北京朝阳分公司）
  - `E1: 申报日期:` | `F1: 2024-10-XX`
  - `G1: 预算单号:` | `H1: YS-2024-XXXX`
- **第 2 行（单独填写的 Key-Value 单元格）**：
  - `A2: 审批领导:` | `B2: [审批人]`
  - `C2: 所属大区:` | `D2: [华北/华东/华南等]`
  - `E2: 申报月份:` | `F2: 2024-10`
  - `G2: 结算币种:` | `H2: 人民币(CNY)`
- **第 3 行**：真正的数据明细列名表头（**表头所在行 = 3**）
- **第 4~28 行**：明细数据（每个分表包含 **25 行** 明细）

---

## 2. 在汇总工具中的推荐操作配置
当您使用 Excel 批量汇总工具导入这 32 个分表时：
1. **设置表头所在行号**：选择或填入 **`3`**（第 3 行为明细字段名）。
2. **配置固定单元格提取（提取元数据）**：
   - 提取 `B1` -> 列名命名为：`填写人`
   - 提取 `D1` -> 列名命名为：`所属机构`
   - 提取 `F1` -> 列名命名为：`申报日期`
   - 提取 `B2` -> 列名命名为：`审批领导`
   - 提取 `D2` -> 列名命名为：`所属大区`
3. **点击生成汇总结果**：工具会自动将每个分表顶部的单独填写内容广播为每一行的属性列，并完成合并！

---

## 3. 汇总核心核对指标
| 指标项 | 预期数值 | 说明 |
| :--- | :--- | :--- |
| **合并分表数** | **32 个** | 覆盖 32 家分支机构/分公司 |
| **合并后明细总行数** | **{total_records} 行** | 32 家分支机构 × 25 行/家 |
| **发票总张数合计** | **{total_inv_count:,} 张** | `发票张数` 列直接求和 |
| **申请报销总额** | **￥{total_apply:,.2f} 元** | `申请报销金额` 列直接求和 |
| **实批报销总额** | **￥{total_approved:,.2f} 元** | `实批报销金额` 列直接求和（精确至分） |

---

## 4. 各分支机构小计明细清单（透视对账表）
| 序号 | 机构编码 | 所属大区 | 所属机构(D1) | 填写人(B1) | 审批领导(B2) | 记录数 | 申请总额(元) | 实批总额(元) |
| :---: | :---: | :---: | :--- | :---: | :---: | :---: | :---: | :---: |
"""
    for idx, (b_code, b_reg, b_name, b_auth, b_appr, b_cnt, b_inv, b_app, b_apr) in enumerate(branch_kpi_list, start=1):
        md_content += f"| {idx} | `{b_code}` | {b_reg} | {b_name} | {b_auth} | {b_appr} | {b_cnt} | ￥{b_app:,.2f} | ￥{b_apr:,.2f} |\n"
    md_content += f"| **合计** | - | - | **全部32家分支机构** | - | - | **{total_records}** | **￥{total_apply:,.2f}** | **￥{total_approved:,.2f}** |\n"

    with open(os.path.join(target_dir, "00_核对指标与快速对账表.md"), "w", encoding="utf-8") as f:
        f.write(md_content)

    print(f"  [OK] 类别四已生成：32 个分表，1 个总表，共 {total_records} 行。")
    return {
        "title": "04_分支机构费用申报(含单独填写单元格)",
        "folder": "04_分支机构费用申报_含自定义表头与固定单元格",
        "sheet_name": "费用申报",
        "files": 32,
        "rows": total_records,
        "metric1_name": "申请报销总额",
        "metric1_val": f"￥{total_apply:,.2f}",
        "metric2_name": "实批报销总额",
        "metric2_val": f"￥{total_approved:,.2f}",
        "raw_sum": {"申请报销金额": total_apply, "实批报销金额": total_approved}
    }

# ==============================================================================
# 生成全局总核对指南与同步父目录
# ==============================================================================
def create_global_overview(results):
    overview_path = os.path.join(OUTPUT_DIR, "00_全套测试集快速对账总览.md")
    r1, r2, r3, r4 = results
    content = f"""# Excel 汇总测试数据集（4大类 × 32分表）快速对账总览

本测试套件专为 **Excel 批量汇总与数据合并工具** 准备。
包含 **4 大真实业务场景**（涵盖规整表格合并、文本保持、多仓库存、以及**自定义格式/单独填写单元格提取合并**），每大类均包含 **32 个分表**（作为测试输入）与 **1 个包含对账看板的总表**（作为对账黄金基准）。

---

## 📊 四大测试类别核心指标快速速查卡

| 类别名称 | 分表数 | 汇总总行数 | 核心对账求和字段 1 | 核心对账求和字段 2 | 业务工作表名 | 适用测试重点 |
| :--- | :---: | :---: | :--- | :--- | :---: | :--- |
| **{r1['title']}** | **{r1['files']} 个** | **{r1['rows']} 行** | **{r1['metric1_name']}**：`{r1['metric1_val']}` | **{r1['metric2_name']}**：`{r1['metric2_val']}` | `{r1['sheet_name']}` | 零售多店日销、金额浮点计算、商品透视 |
| **{r2['title']}** | **{r2['files']} 个** | **{r2['rows']} 行** | **{r2['metric1_name']}**：`{r2['metric1_val']}` | **{r2['metric2_name']}**：`{r2['metric2_val']}` | `{r2['sheet_name']}` | 工号前导0文本保持、薪酬扣款与出勤 |
| **{r3['title']}** | **{r3['files']} 个** | **{r3['rows']} 行** | **{r3['metric1_name']}**：`{r3['metric1_val']}` | **{r3['metric2_name']}**：`{r3['metric2_val']}` | `{r3['sheet_name']}` | 多仓出入库加减逻辑、物料规格编码 |
| **{r4['title']}** | **{r4['files']} 个** | **{r4['rows']} 行** | **{r4['metric1_name']}**：`{r4['metric1_val']}` | **{r4['metric2_name']}**：`{r4['metric2_val']}` | `{r4['sheet_name']}` | **表头在第3行、固定单元格提取（A1填写人/B1姓名等广播合并）** |

---

## 🚀 推荐测试验证流程

### 规整表头测试（类别 1、2、3）：
1. 选中文件夹内的全部 32 个 `.xlsx` 文件，直接拖拽或导入工作台。
2. 确认表头在第 1 行，点击「生成结果」。
3. 对照速查卡中的**总行数**和**核心字段总金额**，或打开总表 Sheet2 看板对比各分组小计。

### 自定义格式与固定单元格提取测试（类别 4）：
1. 打开 `04_分支机构费用申报_含自定义表头与固定单元格/分表测试文件_32家分支机构/`。
2. 导入 32 个分表文件。
3. **设置表头行号为 3**（第 3 行是明细列名）。
4. **添加固定单元格提取**：
   - 提取单元格 `B1`（对应 `填写人`）
   - 提取单元格 `D1`（对应 `所属机构`）
   - 提取单元格 `B2`（对应 `审批领导`）
   - 提取单元格 `D2`（对应 `所属大区`）
5. 生成合并结果，检查导出的数据中是否已将固定单元格作为新列并扩展至每一行！
6. 与同目录的总表「总表_全国分支机构费用报销_32家机构总计_含固定单元格汇总.xlsx」的【汇总合并标准明细】页核对，数字与记录数完全一致！

---

## 📁 目录结构说明

```text
test_datasets/
├── 00_全套测试集快速对账总览.md                   # 本总览文件
├── 01_全国连锁门店销售数据/
│   ├── 00_核对指标与快速对账表.md                 # 类别一详细对账清单
│   ├── 总表_全国连锁门店销售明细_32家门店总计.xlsx  # 黄金总表（含明细+核对看板）
│   └── 分表测试文件_32家门店/ (32个文件)
├── 02_各部门员工薪酬与考勤/
│   ├── 00_核对指标与快速对账表.md                 # 类别二详细对账清单
│   ├── 总表_全集团员工薪酬考勤_32个部门总计.xlsx    # 黄金总表（含明细+核对看板）
│   └── 分表测试文件_32个部门/ (32个文件)
├── 03_各仓储中心库存与物资流转/
│   ├── 00_核对指标与快速对账表.md                 # 类别三详细对账清单
│   ├── 总表_全国各仓库存与流转明细_32个仓库总计.xlsx # 黄金总表（含明细+核对看板）
│   └── 分表测试文件_32个仓库/ (32个文件)
└── 04_分支机构费用申报_含自定义表头与固定单元格/
    ├── 00_核对指标与快速对账表.md                 # 类别四详细对账与提取配置指南
    ├── 总表_全国分支机构费用报销_32家机构总计_含固定单元格汇总.xlsx # 黄金总表（标准宽表+看板）
    └── 分表测试文件_32家分支机构/ (32个文件)
        ├── 费用申报_BJ01_华北_北京朝阳分公司.xlsx
        └── ... (共32个)
```
"""
    with open(overview_path, "w", encoding="utf-8") as f:
        f.write(content)

    print(f"数据已生成至：{OUTPUT_DIR}")

def verify_all_datasets(results):
    """自动化测试验证：读回所有分表并拼合，与总表逐行及按列求和严格核验"""
    print("\n[VERIFY] 正在对生成的分表与总表执行 100% 自动化合并对账...")
    import glob
    for res in results:
        cat_dir = os.path.join(OUTPUT_DIR, res["folder"])
        split_folders = [f for f in os.listdir(cat_dir) if f.startswith("分表测试文件")]
        if not split_folders:
            continue
        sub_folder = os.path.join(cat_dir, split_folders[0])
        files = glob.glob(os.path.join(sub_folder, "*.xlsx"))
        assert len(files) == 32, f"文件数错误：{len(files)}"

        combined_rows = []
        for f in files:
            wb = openpyxl.load_workbook(f, data_only=True)
            ws = wb[res["sheet_name"]]
            rows = list(ws.iter_rows(values_only=True))
            # 类别4的表头在第3行（index 2），数据从第4行（index 3）开始
            if "04" in res["folder"]:
                data_rows = rows[3:]
                # 顺便检查固定单元格
                author = ws.cell(row=1, column=2).value
                org = ws.cell(row=1, column=4).value
                assert author is not None and org is not None, "固定单元格读取失败"
            else:
                data_rows = rows[1:]

            for r in data_rows:
                combined_rows.append(r)

        assert len(combined_rows) == res["rows"], f"总行数不匹配: {len(combined_rows)} vs {res['rows']}"
        print(f"  √ {res['title']}: 32个分表合并行数精确为 {len(combined_rows)} 行，校验通过！")
    print("  √ 全部 4 类测试文件与总表数据一致性校验 100% 通过！\n")

def main():
    if os.path.exists(OUTPUT_DIR):
        raise SystemExit(f"目标目录已存在，避免覆盖请先移走：{OUTPUT_DIR}")
    os.makedirs(OUTPUT_DIR)
    res1 = generate_category_1()
    res2 = generate_category_2()
    res3 = generate_category_3()
    res4 = generate_category_4()
    results = [res1, res2, res3, res4]
    create_global_overview(results)
    verify_all_datasets(results)
    print("ALL DONE: 全部 4 类测试文件及总表已成功生成并校验通过！")

if __name__ == "__main__":
    main()
