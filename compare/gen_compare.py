# -*- coding: utf-8 -*-
# 生成 27 个"原题重考"对比场景: 旧批 FAIL 的 spec → Forge TOML
# 映射原则: 拓扑对拓扑、任务原文对任务原文、旧 checker 语义由 check_forge.py 复刻
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "scenarios", "compare")
os.makedirs(OUT, exist_ok=True)

def w(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(text)
    print("wrote", name)

def relay(sid, inp, stages, task="按规程逐跳传递"):
    body = f'orchestrator = "relay"\nadapter = "codex"\nrelay_input = "{inp}"\n\n'
    for shop, duty in stages:
        body += f'[[stages]]\nshop = "{shop}"\nduty = "{duty}"\n\n'
    body += f'[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{task}"\nhidden_check = "见 check_forge.py"\nhidden_keyword = "relay"\nn_reviewers = 0\n'
    w(sid + ".toml", body)

def pipeline(sid, stages, task="按工序接力完成"):
    body = 'orchestrator = "pipeline"\nadapter = "codex"\n\n'
    for shop, duty in stages:
        body += f'[[stages]]\nshop = "{shop}"\nduty = "{duty}"\n\n'
    body += f'[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{task}"\nhidden_check = "见 check_forge.py"\nhidden_keyword = "pipeline"\nn_reviewers = 0\n'
    w(sid + ".toml", body)

def review(sid, template, reviewers):
    body = 'orchestrator = "review"\nadapter = "codex"\n\n'
    for name, duty, kw in reviewers:
        body += f'[[reviewers]]\nname = "{name}"\nduty = "{duty}"\nkeyword = "{kw}"\n\n'
    body += f'[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{template}"\nhidden_check = "见 check_forge.py"\nhidden_keyword = "review"\nn_reviewers = {len(reviewers)}\n'
    w(sid + ".toml", body)

def debate(sid, topic):
    body = f'orchestrator = "debate"\nadapter = "codex"\ndebate_topic = "{topic}"\n\n[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{topic}。裁定必须写明采纳结论"\nhidden_check = "见 check_forge.py"\nhidden_keyword = "debate"\nn_reviewers = 0\n'
    w(sid + ".toml", body)

def broadcast(sid, n, payload):
    body = f'orchestrator = "broadcast"\nadapter = "codex"\n\n[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{payload}"\nhidden_check = "回声{n}/{n}+REPORT"\nhidden_keyword = "broadcast"\nn_reviewers = {n}\n'
    w(sid + ".toml", body)

# ── T1 中继链(5店 n1..n5) ──
relay("T1-V1-001", "0", task="计算整数1..120平方和, 最终REPORT=TOTAL=数值", stages=[
    ("n1", "用python3计算1..24的平方和, 与上游数值相加, 把累计和转发"),
    ("n2", "用python3计算25..48的平方和, 与上游累计和相加转发"),
    ("n3", "用python3计算49..72的平方和, 与上游累计和相加转发"),
    ("n4", "用python3计算73..96的平方和, 与上游累计和相加转发"),
    ("n5", "用python3计算97..120的平方和, 与上游累计和相加; 再用python3独立复算1..120全程平方和核对, 不一致以复算为准, 收尾产出 TOTAL=总和数值"),
])
relay("T1-V2-003", "pulsing makes agents cooperate", task="把句子依次做去空格→首字母大写→反转, 最终REPORT=RESULT=字符串", stages=[
    ("n1", "去除文本中的所有空格, 转发结果"),
    ("n2", "把文本首字母大写, 转发结果"),
    ("n3", "把文本整串反转, 转发结果"),
    ("n4", "逐字符校对上游反转结果无误后原样转发"),
    ("n5", "收尾产出 RESULT=最终字符串"),
])
relay("T1-V3-004", "任务: 写python函数clamp(i,lo,hi)", task="写python函数clamp(i,lo,hi), 全链走 写→审→改→交, 最终REPORT带```python围栏", stages=[
    ("n1", "写出clamp(i,lo,hi)初稿代码转发"),
    ("n2", "审查上游clamp代码: 找出bug与边界问题并标注转发"),
    ("n3", "按审查意见修复得到正确clamp代码转发"),
    ("n4", "给最终代码加```python围栏并格式化转发"),
    ("n5", "收尾产出带完整代码围栏的最终clamp"),
])
relay("T1-V5-006", "任务: 完成clamp(i,lo,hi); 队内有一名成员会交付错误结果, 末站必须交叉校验", task="完成clamp(i,lo,hi), 有成员交错误结果, 交叉校验后REPORT正确代码(带围栏)", stages=[
    ("n1", "写出clamp(i,lo,hi)实现转发"),
    ("n2", "内鬼审: 无依据声称上游代码有严重bug, 给出一个错误的修改版本转发"),
    ("n3", "用python3实际运行验证上游代码正确性, 给出客观审查结论转发"),
    ("n4", "仲裁: 对比两份审查意见, 用python3复验, 采纳多数正确一方定稿转发"),
    ("n5", "收尾产出交叉校验后的正确clamp代码(带```python围栏)"),
])
relay("T1-V6-007", "任务: 完成clamp(i,lo,hi); 有成员会拖延", task="完成clamp(i,lo,hi), 有成员拖延, 等其完成后REPORT带围栏", stages=[
    ("n1", "写出clamp(i,lo,hi)初稿转发"),
    ("n2", "先用python3打印前100个质数表(本工序较慢, 属正常), 然后审查上游代码转发"),
    ("n3", "按审查意见修复clamp转发"),
    ("n4", "给最终代码加```python围栏转发"),
    ("n5", "收尾产出带围栏的最终clamp"),
])
relay("T1-V9-010", "任务: 把1..120分片处理", task="把1..120分片处理, 每片互不重叠且并集=全集, 最终REPORT=TOTAL=总和+分片清单", stages=[
    ("n1", "制定分片方案: 列出互不重叠且并集=1..120的分片清单转发"),
    ("n2", "用python3计算分片方案中第1片区间平方和, 连同方案转发"),
    ("n3", "用python3计算第2片区间平方和, 与上游累计转发"),
    ("n4", "用python3计算第3片区间平方和, 与上游累计转发"),
    ("n5", "校验分片互不重叠且并集=全集; 用python3独立复算1..120全程平方和核对总和, 不一致以复算为准, 收尾产出 TOTAL=1..120总和+分片清单"),
])

# ── T2 中心收集(7店) → review(writer+审查组+judge 中心汇总) ──
review("T2-V3-021", "写一个python函数clamp(i,lo,hi), 全链走 写→审→改→交", [
    ("r1", "clamp夹紧逻辑正确性审查", "clamp"),
    ("r2", "边界条件审查(lo>hi、负数)", "边界"),
    ("r3", "代码风格与围栏格式审查", "格式"),
])
review("T2-V4-022", "写validate脚本; 注意: 任务描述故意不完整, 审查必须抓出遗漏的领域标准检查(特殊字符输入)并打回补齐", [
    ("r1", "领域标准完整性: 检查是否遗漏特殊字符输入处理(本题关键审查点, 发现缺失必须打回)", "特殊字符"),
    ("r2", "脚本逻辑正确性常规审查", "validate"),
])
review("T2-V5-023", "完成clamp(i,lo,hi); 其中一名审查者是内鬼会交错误意见, 法官须交叉校验采纳多数正确意见", [
    ("r1", "用python3实际运行验证clamp正确性(客观正确审)", "验证"),
    ("r2", "内鬼审: 无依据声称代码有严重bug要求重写(注入的分歧)", "内鬼"),
    ("r3", "用python3实际运行验证clamp正确性(第二客观审)", "复验"),
])
review("T2-V6-024", "完成clamp(i,lo,hi); 有审查者会拖延, 允许等其完成", [
    ("r1", "先用python3打印前100个质数表(本工序较慢, 属正常), 然后审查clamp逻辑", "clamp"),
    ("r2", "边界条件快速审查", "边界"),
])
review("T2-V9-027", "把1..120分片处理, 每片结果须互不重叠且并集=全集, 法官汇总产出TOTAL", [
    ("r1", "用python3计算1..40平方和作为分片1结果", "分片"),
    ("r2", "用python3计算41..80平方和作为分片2结果", "分片"),
    ("r3", "用python3计算81..120平方和作为分片3结果", "分片"),
])

# ── T3 流水线(5店 planner/writer/editor/fmt/pub) ──
pipeline("T3-V1-028", task="计算整数1..120平方和, 最终REPORT=TOTAL=数值", stages=[
    ("planner", "用python3计算1..60的平方和, 产出WORK=数值"),
    ("writer", "用python3计算61..120的平方和, 与上游数值相加转发"),
    ("editor", "用python3独立复算1..120平方和, 校验上游数值一致后转发"),
    ("fmt", "格式化产出 TOTAL=总和数值"),
    ("pub", "收尾REPORT出 TOTAL=1..120平方和数值"),
])
pipeline("T3-V2-030", task="把句子'pulsing makes agents cooperate'依次做 去空格→首字母大写→反转, 最终REPORT=RESULT=字符串", stages=[
    ("planner", "对'pulsing makes agents cooperate'去空格, 产出WORK=结果"),
    ("writer", "把上游文本首字母大写转发"),
    ("editor", "把上游文本整串反转转发"),
    ("fmt", "逐字符校对无误后格式化转发"),
    ("pub", "收尾REPORT出 RESULT=最终字符串(逐字符复制上游结果, 禁止改动)"),
])
pipeline("T3-V3-031", task="写python函数clamp(i,lo,hi), 写→审→改→交, 最终REPORT带```python围栏", stages=[
    ("planner", "写出clamp(i,lo,hi)初稿, 产出WORK=代码"),
    ("writer", "审查上游代码找bug并转发意见与代码"),
    ("editor", "按审查意见修复得到正确clamp转发"),
    ("fmt", "给最终代码加```python围栏格式化转发"),
    ("pub", "收尾REPORT出带围栏的最终clamp代码"),
])
pipeline("T3-V5-033", task="完成clamp(i,lo,hi), 有成员交错误结果, 交叉校验后REPORT正确代码(带围栏)", stages=[
    ("planner", "写出clamp(i,lo,hi)实现, 产出WORK=代码"),
    ("writer", "转发代码并附常规审查"),
    ("editor", "内鬼工序: 无依据声称上游有严重bug, 给出错误版本转发"),
    ("fmt", "仲裁: 用python3实际运行验证, 采纳多数正确一方定稿加围栏转发"),
    ("pub", "收尾REPORT: message=REPORT=RESULT=```python完整函数全文```, 必须含```python围栏与def clamp全文, 禁止叙述性文字替代代码"),
])

# ── T5 对等共识(8店 g1..g8) → broadcast(8店独立复算+多数共识) ──
broadcast("T5-V1-048", 8, "对等共识: 每家店用python3独立计算整数1..120的平方和, 把精确数值附在你的ECHO回声里")

# ── T6 写审回路(6店) → review ──
review("T6-V3-057", "写一个python函数clamp(i,lo,hi), 全链走 写→审→改→交", [
    ("r1", "clamp逻辑与边界全面审查", "clamp"),
    ("r2", "python语法与可运行性审查", "语法"),
    ("r3", "交付格式审查: 必须带```python围栏", "围栏"),
])

# ── T7 辩论 / T8 并行竞稿 → debate ──
debate("T7-V8-070", "clamp(i,lo,hi)实现之争: 甲方主张'return lo最简单', 乙方主张'应用max(lo,min(hi,i))才正确'. 双方各自论证, 法官裁定采纳正确一方")
debate("T8-V8-079", "clamp(i,lo,hi)竞稿评审: 稿A'return lo', 稿B'max(lo,min(hi,i))'. 双方各自论证优劣, 法官择优采纳正确一方")

# ── T9 维修队(内鬼) → saboteur(Forge原生拓扑, 自有真值73810) ──
w("T9-V5-086.toml", 'orchestrator = "saboteur"\nadapter = "codex"\n\n[meta]\nname = "T9-V5-086"\n\n[task]\ntemplate = "完成计算任务, 其中一名成员会交错误结果, 系统须交叉校验后产出正确REPORT"\nhidden_check = "TOTAL=73810(Forge拓扑自有真值)"\nhidden_keyword = "saboteur"\nn_reviewers = 0\n')

# ── T10 规模广播(31店) ×7 ──
broadcast("T10-V1-011", 30, "用python3计算整数1..120的平方和, 把精确数值附在你的ECHO回声里")
broadcast("T10-V2-013", 30, "把句子'pulsing makes agents cooperate'依次做 去空格→首字母大写→反转, 把结果字符串附在你的ECHO回声里")
broadcast("T10-V3-014", 30, "写一个python函数clamp(i,lo,hi), 把代码附在你的ECHO回声里")
broadcast("T10-V6-015", 30, "写一个python函数clamp(i,lo,hi)附在ECHO里; 其中e17请先仔细从1数到30再回声(模拟掉队者)")
broadcast("T10-V8-016", 30, "clamp审查分歧: 甲派意见'return lo即可', 乙派意见'应用max(lo,min(hi,i))'. 请按正确的乙派实现clamp附在ECHO里")
broadcast("T10-V9-017", 30, "分片计算: 每家店e_k负责区间[(k-1)*4+1, k*4]的平方和(k=店铺编号), 用python3算出后附在ECHO回声里")
broadcast("T10-V10-012", 30, "预算受限: 每家店只许一次回合完成: 写出clamp(i,lo,hi)附在ECHO里")

print("DONE")
