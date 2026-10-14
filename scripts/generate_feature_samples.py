# -*- coding: utf-8 -*-
"""
生成本次功能改动（跨表匹配、多结果表拆分、多表Excel导出、多文件夹与临时文件过滤）的10个专项测试样本。
存储路径：test_datasets/05_新功能专项测试_跨表匹配与多表拆分/
"""

import os
import sys
import datetime
import openpyxl
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from openpyxl.utils import get_column_letter

# 配置控制台输出 UTF-8
if sys.stdout.encoding.lower() != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGET_DIR = os.path.join(BASE_DIR, "test_datasets", "05_新功能专项测试_跨表匹配与多表拆分")
os.makedirs(TARGET_DIR, exist_ok=True)

# 样式常量
FONT_HEADER = Font(name="微软雅黑", size=10, bold=True, color="FFFFFF")
FONT_DATA = Font(name="微软雅黑", size=10, color="000000")
FONT_TITLE = Font(name="微软雅黑", size=12, bold=True, color="1E3A8A")

BORDER_THIN = Border(
    left=Side(style='thin', color='CBD5E1'),
    right=Side(style='thin', color='CBD5E1'),
    top=Side(style='thin', color='CBD5E1'),
    bottom=Side(style='thin', color='CBD5E1')
)

FILL_BLUE = PatternFill(start_color="2563EB", end_color="2563EB", fill_type="solid")
FILL_EMERALD = PatternFill(start_color="059669", end_color="059669", fill_type="solid")
FILL_PURPLE = PatternFill(start_color="7C3AED", end_color="7C3AED", fill_type="solid")
FILL_AMBER = PatternFill(start_color="D97706", end_color="D97706", fill_type="solid")
FILL_SLATE = PatternFill(start_color="475569", end_color="475569", fill_type="solid")

def format_sheet(ws, fill=FILL_BLUE):
    """设置表头样式、数据字体、边框及自适应列宽"""
    ws.views.sheetView[0].showGridLines = True
    for col_idx, col in enumerate(ws.columns, start=1):
        max_len = 0
        for row_idx, cell in enumerate(col, start=1):
            cell.border = BORDER_THIN
            if row_idx == 1:
                cell.font = FONT_HEADER
                cell.fill = fill
                cell.alignment = Alignment(horizontal="center", vertical="center", wrap_text=True)
            else:
                cell.font = FONT_DATA
                if isinstance(cell.value, (int, float)):
                    cell.alignment = Alignment(horizontal="right", vertical="center")
                elif isinstance(cell.value, (datetime.date, datetime.datetime)):
                    cell.alignment = Alignment(horizontal="center", vertical="center")
                    cell.number_format = 'yyyy-mm-dd'
                elif isinstance(cell.value, bool):
                    cell.alignment = Alignment(horizontal="center", vertical="center")
                else:
                    cell.alignment = Alignment(horizontal="left", vertical="center")
            val_str = str(cell.value or '')
            # 中文字符按双宽度计算
            char_len = sum(2 if ord(c) > 127 else 1 for c in val_str)
            if char_len > max_len:
                max_len = char_len
        col_letter = get_column_letter(col_idx)
        ws.column_dimensions[col_letter].width = max(max_len + 4, 14)
    ws.row_dimensions[1].height = 26

def make_sample_01():
    """样本 01：标准零售日结与商品信息字典匹配（基础 1:1 跨表查找）"""
    wb = openpyxl.Workbook()
    # Sheet 1: 销售明细
    ws1 = wb.active
    ws1.title = "销售明细"
    ws1.append(["销售单号", "门店名称", "商品编码", "销售数量", "销售单价"])
    rows1 = [
        ["SO20261001-01", "上海静安店", "SPU-101", 15, 68.0],
        ["SO20261001-02", "上海静安店", "SPU-102", 8, 128.0],
        ["SO20261001-03", "上海静安店", "SPU-103", 20, 45.0],
        ["SO20261001-04", "上海静安店", "SPU-104", 12, 88.0],
        ["SO20261001-05", "上海静安店", "SPU-105", 5, 260.0],
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    # Sheet 2: 商品信息字典
    ws2 = wb.create_sheet(title="商品信息字典")
    ws2.append(["商品编码", "商品全称", "品类分组", "规格型号", "标准进货价"])
    rows2 = [
        ["SPU-101", "冷萃浓缩咖啡液", "咖啡饮品", "500ml/瓶", 38.0],
        ["SPU-102", "云南高山精品日晒咖啡豆", "咖啡原豆", "250g/包", 75.0],
        ["SPU-103", "燕麦植物奶基底", "乳品辅料", "1L/盒", 22.0],
        ["SPU-104", "便携手冲随行杯", "咖啡周边", "400ml/个", 45.0],
        ["SPU-105", "经典意式手摇磨豆机", "咖啡器具", "不锈钢精钢磨芯", 160.0],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_EMERALD)

    out = os.path.join(TARGET_DIR, "样本01_销售明细与商品字典_基础跨表匹配.xlsx")
    wb.save(out)
    return out

def make_sample_02():
    """样本 02：员工考勤薪酬与前导零工号字典匹配（文本型主键前导0不丢失）"""
    wb = openpyxl.Workbook()
    # Sheet 1: 月度考勤明细
    ws1 = wb.active
    ws1.title = "月度考勤明细"
    ws1.append(["考勤月份", "员工工号", "出勤天数", "加班小时", "请假天数"])
    rows1 = [
        ["2026-09", "00101", 22, 10.5, 0],
        ["2026-09", "00102", 21, 4.0, 1],
        ["2026-09", "00205", 22, 16.0, 0],
        ["2026-09", "00308", 20, 0.0, 2],
        ["2026-09", "00412", 22, 8.0, 0],
    ]
    for r in rows1:
        ws1.append(r)
    # 强制将工号列设为文本格式，保留前导0
    for cell in ws1['B'][1:]:
        cell.number_format = '@'
    format_sheet(ws1, FILL_BLUE)

    # Sheet 2: 员工人事档案
    ws2 = wb.create_sheet(title="员工人事档案")
    ws2.append(["工号", "员工姓名", "所属部门", "岗位职级", "基本工资"])
    rows2 = [
        ["00101", "陈晨", "技术研发部", "P6-高级前端", 18500],
        ["00102", "林悦", "技术研发部", "P7-架构师", 26000],
        ["00205", "张子涵", "市场运营部", "M1-运营主管", 16000],
        ["00308", "王浩宇", "财务核算部", "P5-会计", 11500],
        ["00412", "赵欣怡", "人力资源部", "P5-HRBP", 12000],
    ]
    for r in rows2:
        ws2.append(r)
    for cell in ws2['A'][1:]:
        cell.number_format = '@'
    format_sheet(ws2, FILL_PURPLE)

    out = os.path.join(TARGET_DIR, "样本02_员工考勤与人员档案_前导零工号匹配.xlsx")
    wb.save(out)
    return out

def make_sample_03():
    """样本 03：未匹配数据容错与留空策略（缺失关联值：留空并生成 Issue 记录）"""
    wb = openpyxl.Workbook()
    ws1 = wb.active
    ws1.title = "销售明细"
    ws1.append(["销售单号", "门店名称", "商品编码", "销售数量", "销售单价"])
    rows1 = [
        ["SO20261002-01", "广州天河店", "SPU-101", 10, 68.0],
        ["SO20261002-02", "广州天河店", "SPU-999", 2, 99.0],   # 未收录新编码 1
        ["SO20261002-03", "广州天河店", "SPU-103", 5, 45.0],
        ["SO20261002-04", "广州天河店", "SPU-888", 1, 150.0],  # 未收录新编码 2
        ["SO20261002-05", "广州天河店", "SPU-102", 4, 128.0],
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    ws2 = wb.create_sheet(title="商品字典")
    ws2.append(["商品编码", "商品全称", "品类分类"])
    rows2 = [
        ["SPU-101", "冷萃浓缩咖啡液", "咖啡饮品"],
        ["SPU-102", "云南高山精品日晒咖啡豆", "咖啡原豆"],
        ["SPU-103", "燕麦植物奶基底", "乳品辅料"],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_AMBER)

    out = os.path.join(TARGET_DIR, "样本03_销售明细与字典_含未收录编码_容错留空测试.xlsx")
    wb.save(out)
    return out

def make_sample_04():
    """样本 04：缺失关联值强校验中断（缺失关联值：严格阻断报错）"""
    wb = openpyxl.Workbook()
    ws1 = wb.active
    ws1.title = "资产领用明细"
    ws1.append(["申请单号", "领用人工号", "资产编码", "领用数量"])
    rows1 = [
        ["AST-REQ-001", "00101", "FIX-LAPTOP-01", 1],
        ["AST-REQ-002", "00102", "FIX-MONITOR-02", 2],
        ["AST-REQ-003", "00205", "FIX-UNKNOWN-99", 1],  # 异常编码，阻断测试
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    ws2 = wb.create_sheet(title="资产标准台账")
    ws2.append(["资产编码", "资产名称", "规格型号", "折旧年限"])
    rows2 = [
        ["FIX-LAPTOP-01", "轻薄商务笔记本电脑", "14寸/32G/1T", 3],
        ["FIX-MONITOR-02", "4K高清设计显示器", "27寸/Type-C 90W", 5],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_SLATE)

    out = os.path.join(TARGET_DIR, "样本04_固定资产领用_含未知编码_强校验阻断测试.xlsx")
    wb.save(out)
    return out

def make_sample_05():
    """样本 05：参考表重复主键冲突报错（字典重复值：严格校验中断）"""
    wb = openpyxl.Workbook()
    ws1 = wb.active
    ws1.title = "物料入库明细"
    ws1.append(["入库批次", "物料编码", "入库数量"])
    rows1 = [
        ["IN-202610-01", "MAT-A01", 500],
        ["IN-202610-02", "MAT-A02", 200],
        ["IN-202610-03", "MAT-A03", 100],
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    ws2 = wb.create_sheet(title="物料标准表")
    ws2.append(["物料编码", "物料名称", "采购单价"])
    rows2 = [
        ["MAT-A01", "食品级耐温纸杯", 0.35],
        ["MAT-A01", "食品级耐温纸杯(改版加厚)", 0.42],  # 重复主键
        ["MAT-A02", "环保可降解吸管", 0.08],
        ["MAT-A03", "双层瓦楞杯套", 0.12],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_AMBER)

    out = os.path.join(TARGET_DIR, "样本05_物料入库单_字典含重复主键_重复冲突校验测试.xlsx")
    wb.save(out)
    return out

def make_sample_06():
    """样本 06：参考表重复主键策略选择（取第一条 vs 取最后一条）"""
    wb = openpyxl.Workbook()
    ws1 = wb.active
    ws1.title = "领料结算明细"
    ws1.append(["领料单号", "供应商代码", "领用数量"])
    rows1 = [
        ["REQ-01", "VEND-801", 100],
        ["REQ-02", "VEND-802", 200],
        ["REQ-03", "VEND-803", 150],
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    ws2 = wb.create_sheet(title="供应商价目历史库")
    ws2.append(["供应商代码", "供应商名称", "协议单价", "版本批注"])
    rows2 = [
        ["VEND-801", "晨光包装厂", 45.0, "2025年旧版本价（第一条）"],
        ["VEND-801", "晨光包装厂", 52.0, "2026年最新调价（最后一条）"],
        ["VEND-802", "华夏纸业", 30.0, "标准统一报价"],
        ["VEND-803", "绿源降解科技", 68.0, "2025初始报价（第一条）"],
        ["VEND-803", "绿源降解科技", 62.0, "2026大宗阶梯优惠（最后一条）"],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_EMERALD)

    out = os.path.join(TARGET_DIR, "样本06_领料结算表_字典多版本记录_取首条与取末条测试.xlsx")
    wb.save(out)
    return out

def make_sample_07():
    """样本 07：跨表查找带出日期与布尔/数值等复杂类型（类型智能推断）"""
    wb = openpyxl.Workbook()
    ws1 = wb.active
    ws1.title = "项目节点进度"
    ws1.append(["合同编号", "当前里程碑", "当期开票金额"])
    rows1 = [
        ["HT-2026-001", "初验完成", 85000.0],
        ["HT-2026-002", "终验上线", 140000.0],
        ["HT-2026-003", "需求确认", 32000.0],
    ]
    for r in rows1:
        ws1.append(r)
    format_sheet(ws1, FILL_BLUE)

    ws2 = wb.create_sheet(title="合同主协议信息")
    ws2.append(["合同编号", "签约日期", "是否含质保", "质保金额"])
    rows2 = [
        ["HT-2026-001", datetime.date(2026, 2, 18), True, 8500.0],
        ["HT-2026-002", datetime.date(2026, 4, 12), False, 0.0],
        ["HT-2026-003", datetime.date(2026, 8, 5), True, 3200.0],
    ]
    for r in rows2:
        ws2.append(r)
    format_sheet(ws2, FILL_PURPLE)

    out = os.path.join(TARGET_DIR, "样本07_项目进度看板_带出签约日期与质保状态_类型推断测试.xlsx")
    wb.save(out)
    return out

def make_sample_08():
    """样本 08：多业务 Sheet 混合单簿（一键按 Sheet 分开汇总测试，生成2个分公司文件）"""
    files = []
    branches = [
        ("华北分公司", "BJ", [
            ("门店营业收入", ["门店编号", "实收金额", "订单笔数", "客单均价"], [
                ["BJ-STORE-01", 12560.0, 310, 40.5],
                ["BJ-STORE-02", 18920.0, 420, 45.0],
                ["BJ-STORE-03", 9450.0, 225, 42.0],
            ]),
            ("日常行政支出", ["申报部门", "经办人", "报销金额", "费用摘要"], [
                ["市场运营", "张伟", 1580.0, "三季度线下推广展位物料费"],
                ["技术研发", "王强", 860.0, "服务器故障应急上门维保"],
            ]),
            ("客诉服务台账", ["工单单号", "客户手机", "投诉类别", "赔付金额"], [
                ["TS-2026-001", "13800138001", "配送超时", 20.0],
                ["TS-2026-002", "13900139002", "包装破损", 35.0],
            ])
        ]),
        ("华东分公司", "SH", [
            ("门店营业收入", ["门店编号", "实收金额", "订单笔数", "客单均价"], [
                ["SH-STORE-01", 21400.0, 480, 44.5],
                ["SH-STORE-02", 16300.0, 360, 45.2],
            ]),
            ("日常行政支出", ["申报部门", "经办人", "报销金额", "费用摘要"], [
                ["行政后勤", "刘芳", 2400.0, "职场净水滤芯与耗材更换"],
                ["门店督导", "周敏", 1200.0, "门店巡检城际高铁差旅"],
            ]),
            ("客诉服务台账", ["工单单号", "客户手机", "投诉类别", "赔付金额"], [
                ["TS-2026-003", "13700137003", "饮品口味偏差", 15.0],
            ])
        ])
    ]

    for name, code, sheets in branches:
        wb = openpyxl.Workbook()
        for idx, (sheet_title, headers, data) in enumerate(sheets):
            ws = wb.active if idx == 0 else wb.create_sheet(title=sheet_title)
            ws.title = sheet_title
            ws.append(headers)
            for row in data:
                ws.append(row)
            format_sheet(ws, FILL_BLUE if idx == 0 else (FILL_AMBER if idx == 1 else FILL_EMERALD))
        out_path = os.path.join(TARGET_DIR, f"样本08_综合运营月报_按Sheet分开汇总测试_{name}.xlsx")
        wb.save(out_path)
        files.append(out_path)
    return files

def make_sample_09():
    """样本 09：多结果表导出与表名合规净化测试（包含保留表名 History、长表名与同名检测）"""
    wb = openpyxl.Workbook()
    # Sheet 1: History（Excel 系统保留名称，splitBySheet 应自动转为 'History 汇总'）
    ws1 = wb.active
    ws1.title = "History"
    ws1.append(["测试项目", "指标数值", "规则说明"])
    ws1.append(["Excel系统保留词测试", 100, "History为保留字，splitBySheet会自动重命名为'History 汇总'"])
    format_sheet(ws1, FILL_SLATE)

    # Sheet 2: 区域销售核心表
    ws2 = wb.create_sheet(title="区域销售核心表")
    ws2.append(["区域", "销售额", "达成率"])
    ws2.append(["华东区", 320000.0, "105%"])
    ws2.append(["华北区", 280000.0, "98%"])
    format_sheet(ws2, FILL_PURPLE)

    # Sheet 3: 区域销售核心表备用（同名相似业务表）
    ws3 = wb.create_sheet(title="区域销售分析表")
    ws3.append(["分析维度", "客单价", "退款率"])
    ws3.append(["线上渠道", 88.5, "1.2%"])
    ws3.append(["线下门店", 125.0, "0.5%"])
    format_sheet(ws3, FILL_AMBER)

    # Sheet 4: 边界长度表名（31个有效字符，Excel单表名上限）
    boundary_title = "财务部度末全渠道运营利润与成本核算表"[:31]
    ws4 = wb.create_sheet(title=boundary_title)
    ws4.append(["科目", "金额"])
    ws4.append(["净利润", 950000.0])
    format_sheet(ws4, FILL_EMERALD)

    out = os.path.join(TARGET_DIR, "样本09_多结果表导出与名称净化测试.xlsx")
    wb.save(out)
    return out

def make_sample_10():
    """样本 10：文件夹导入抗干扰（包含 Excel 临时锁文件 ~$ 与真实文件）"""
    folder10 = os.path.join(TARGET_DIR, "样本10_多文件夹导入与临时文件抗干扰")
    folder_a = os.path.join(folder10, "门店文件夹_01_上海分部")
    folder_b = os.path.join(folder10, "门店文件夹_02_北京分部")
    os.makedirs(folder_a, exist_ok=True)
    os.makedirs(folder_b, exist_ok=True)

    # 文件夹 A 真实文件
    wb_a = openpyxl.Workbook()
    ws_a = wb_a.active
    ws_a.title = "销售明细"
    ws_a.append(["门店编码", "销售额", "客流"])
    ws_a.append(["SH-01", 15800.0, 320])
    format_sheet(ws_a, FILL_BLUE)
    real_a = os.path.join(folder_a, "2026年10月销售报表_静安店.xlsx")
    wb_a.save(real_a)

    # 文件夹 A 临时锁文件（模拟 ~$ 产生的锁文件）
    lock_a = os.path.join(folder_a, "~$2026年10月销售报表_静安店.xlsx")
    with open(lock_a, "wb") as f:
        f.write(b"\x00\x00\x00\x00OFFICE_LOCK_RECORD\x00\x00")

    # 文件夹 B 真实文件
    wb_b = openpyxl.Workbook()
    ws_b = wb_b.active
    ws_b.title = "销售明细"
    ws_b.append(["门店编码", "销售额", "客流"])
    ws_b.append(["BJ-01", 18200.0, 410])
    format_sheet(ws_b, FILL_EMERALD)
    real_b = os.path.join(folder_b, "2026年10月销售报表_朝阳店.xlsx")
    wb_b.save(real_b)

    # 文件夹 B 临时锁文件
    lock_b = os.path.join(folder_b, "~$2026年10月销售报表_朝阳店.xlsx")
    with open(lock_b, "wb") as f:
        f.write(b"\x00\x00\x00\x00OFFICE_LOCK_RECORD\x00\x00")

    return folder10

def main():
    print("开始生成本次改动专属 10 个测试样本...")
    f1 = make_sample_01()
    print(f"[OK] 样本 01 生成成功: {os.path.basename(f1)}")
    f2 = make_sample_02()
    print(f"[OK] 样本 02 生成成功: {os.path.basename(f2)}")
    f3 = make_sample_03()
    print(f"[OK] 样本 03 生成成功: {os.path.basename(f3)}")
    f4 = make_sample_04()
    print(f"[OK] 样本 04 生成成功: {os.path.basename(f4)}")
    f5 = make_sample_05()
    print(f"[OK] 样本 05 生成成功: {os.path.basename(f5)}")
    f6 = make_sample_06()
    print(f"[OK] 样本 06 生成成功: {os.path.basename(f6)}")
    f7 = make_sample_07()
    print(f"[OK] 样本 07 生成成功: {os.path.basename(f7)}")
    f8 = make_sample_08()
    print(f"[OK] 样本 08 生成成功: {len(f8)} 个分公司综合月报")
    f9 = make_sample_09()
    print(f"[OK] 样本 09 生成成功: {os.path.basename(f9)}")
    f10 = make_sample_10()
    print(f"[OK] 样本 10 生成成功: {os.path.basename(f10)} (含多文件夹与~$临时锁文件)")
    print("\n全套 10 个测试样本生成完毕！目录：", TARGET_DIR)

if __name__ == "__main__":
    main()
