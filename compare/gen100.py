# -*- coding: utf-8 -*-
# 全量100场景生成器: /mnt/d/tmp/pulsing-tests/specs 的原文任务 → Forge TOML
# 拓扑→编排器映射 + 陷阱(trap)职责模板; 任务文本一律用旧批 spec 原文(原题重考)
import json, glob, os, re

OUT = os.path.join(os.path.dirname(__file__), "..", "scenarios", "full100")
os.makedirs(OUT, exist_ok=True)

def q(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')

def relay_toml(sid, task, stages, inp="0"):
    # 注意: relay_input 必须在 [meta] 内(代码读 sc.meta.relay_input; 顶层会被静默忽略)
    body = f'orchestrator = "relay"\nadapter = "codex"\n\n'
    for shop, duty in stages:
        body += f'[[stages]]\nshop = "{shop}"\nduty = "{q(duty)}"\n\n'
    body += f'[meta]\nname = "{sid}"\nrelay_input = "{q(inp)}"\n\n[task]\ntemplate = "{q(task)}"\nhidden_check = "check100"\nhidden_keyword = "relay"\nn_reviewers = 0\n'
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

def pipeline_toml(sid, task, stages):
    body = 'orchestrator = "pipeline"\nadapter = "codex"\n\n'
    for shop, duty in stages:
        body += f'[[stages]]\nshop = "{shop}"\nduty = "{q(duty)}"\n\n'
    body += f'[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{q(task)}"\nhidden_check = "check100"\nhidden_keyword = "pipeline"\nn_reviewers = 0\n'
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

def review_toml(sid, task, reviewers):
    body = 'orchestrator = "review"\nadapter = "codex"\n\n'
    for name, duty in reviewers:
        body += f'[[reviewers]]\nname = "review_{name}"\nduty = "{q(duty)}"\nkeyword = "kw"\n\n'
    body += f'[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{q(task)}"\nhidden_check = "check100"\nhidden_keyword = "review"\nn_reviewers = {len(reviewers)}\n'
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

def debate_toml(sid, task, topic):
    body = f'orchestrator = "debate"\nadapter = "codex"\ndebate_topic = "{q(topic)}"\n\n[meta]\nname = "{sid}"\n\n[task]\ntemplate = "{q(task)}"\nhidden_check = "check100"\nhidden_keyword = "debate"\nn_reviewers = 0\n'
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

def saboteur_toml(sid, task):
    body = 'orchestrator = "saboteur"\nadapter = "codex"\n\n[meta]\nname = "%s"\n\n[task]\ntemplate = "%s"\nhidden_check = "check100"\nhidden_keyword = "saboteur"\nn_reviewers = 0\n' % (sid, q(task))
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

def broadcast_toml(sid, task, payload, n):
    body = 'orchestrator = "broadcast"\nadapter = "codex"\n\n[meta]\nname = "%s"\n\n[task]\ntemplate = "%s"\nhidden_check = "check100"\nhidden_keyword = "broadcast"\nn_reviewers = %d\n' % (sid, q(payload), n)
    open(os.path.join(OUT, sid + ".toml"), "w", encoding="utf-8").write(body)

# ── 陷阱职责模板 ──
SUM = "计算整数1..120的平方和, 分工自定, 最终REPORT=TOTAL=数值"
FAULT_FIX = "用python3独立复算并裁定, 不轻信任何单源"

def relay_stages(trap, task):
    if trap == "数值区间和":
        return [("n1", "用python3计算1..24平方和, 转发正文必须含一行标记 SEG=1..24=<你的数值>"),
                ("n2", "用python3计算25..48平方和, 转发正文必须含一行标记 SEG=25..48=<你的数值>(上游SEG行原样保留)"),
                ("n3", "用python3计算49..72平方和, 转发正文必须含一行标记 SEG=49..72=<你的数值>(上游SEG行原样保留)"),
                ("n4", "用python3计算73..96平方和, 转发正文必须含一行标记 SEG=73..96=<你的数值>(上游SEG行原样保留)"),
                ("n5", "用python3计算97..120平方和, 先发 SEG=97..120=<数值> 给n5自己留账; 然后用 pulsing_recall(query='SEG=') 搜索账本全部5段SEG标记, 用python3把5个数值求和, 收尾产出 TOTAL=求和结果+分段清单(禁止心算)")]
    if trap == "字符串变换链":
        return [("n1", "去除文本所有空格转发"), ("n2", "首字母大写转发"),
                ("n3", "整串反转转发"), ("n4", "逐字符校对无误后原样转发"),
                ("n5", "收尾产出 RESULT=最终字符串(逐字符复制上游, 禁止改动)")]
    if trap in ("代码生成+审",):
        return [("n1", "写出clamp(i,lo,hi)初稿转发"), ("n2", "审查上游找bug并附意见转发"),
                ("n3", "按审查意见修复定稿转发"), ("n4", "加```python围栏格式化转发"),
                ("n5", "收尾产出带围栏的最终代码全文")]
    if trap == "埋雷":
        return [("n1", "严格按任务字面写validate脚本初稿(任务没提的一律不做)转发"),
                ("n2", "领域完整性审查: 检查是否遗漏领域标准检查(如特殊字符/空值/越界), 发现缺失必须打回并在信中写明缺失项"),
                ("n3", "按打回意见补齐缺失检查后转发"), ("n4", "加围栏定稿转发"),
                ("n5", "收尾产出带围栏全文+已补齐的检查清单")]
    if trap == "内鬼":
        return [("n1", "写出clamp(i,lo,hi)实现转发"),
                ("n2", "故障注入器: 不计算, 原样上报预设载荷 RESULT=breaker||区段||50000"),
                ("n3", "用python3实际运行验证上游代码, 客观审查转发"),
                ("n4", "仲裁: 用python3复验, 采纳多数正确一方定稿加围栏转发"),
                ("n5", "收尾产出交叉校验后的正确代码(带围栏)")]
    if trap == "掉队者":
        return [("n1", "写出clamp(i,lo,hi)初稿转发"),
                ("n2", "先用python3打印前100个质数表(本工序慢, 属正常), 然后审查上游转发"),
                ("n3", "按意见修复转发"), ("n4", "加围栏转发"), ("n5", "收尾产出带围栏全文")]
    if trap == "模糊规格":
        return [("n1", "列出任务描述中的模糊点, 对每点选定一个合理默认并说明理由转发"),
                ("n2", "按该默认执行任务转发"), ("n3", "校对执行结果与默认一致转发"),
                ("n4", "格式化定稿转发"), ("n5", "收尾产出结果全文+所采用的默认假设清单")]
    if trap == "冲突评审":
        return [("n1", "按甲方意见(用max(lo,min(hi,i))实现)产出v1转发"),
                ("n2", "乙方审查: 主张应改为直接return lo并给出理由转发"),
                ("n3", "用python3实测两组输入裁定哪方正确, 采纳正确方转发"),
                ("n4", "定稿加围栏转发"), ("n5", "收尾产出采纳结论+最终代码(带围栏)")]
    if trap == "分片一致":
        return [("n1", "制定1..120分片方案(建议1..24/25..48/49..72/73..96/97..120): 互不重叠且并集=全集, 清单转发"),
                ("n2", "用python3计算片1平方和, 转发含 SEG=片1区间=<数值>"), ("n3", "用python3计算片2平方和, 转发含 SEG=片2区间=<数值>(上游SEG保留)"),
                ("n4", "用python3计算片3平方和, 转发含 SEG=片3区间=<数值>(上游SEG保留)"),
                ("n5", "校验分片不重叠并集=全集; 用 pulsing_recall(query='SEG=') 搜回账本全部分段, python求和(含你算的剩余段), 收尾产出 TOTAL=求和+分片清单(禁止心算)")]
    if trap == "预算压力":
        return [("n1", "用python3完成任务转发"), ("n2", "快速核验转发"),
                ("n3", "收尾产出结果(附关键产出全文)")]
    return [("n1", "按任务执行转发"), ("n2", "校对转发"), ("n3", "收尾产出结果全文"),
            ("n4", "格式化转发"), ("n5", "收尾产出最终结果")]

def review_reviewers(trap, task):
    if trap == "数值区间和":
        return [("r1", "用python3计算1..40平方和作为分片1"), ("r2", "用python3计算41..80平方和作为分片2"),
                ("r3", "用python3计算81..120平方和作为分片3")]
    if trap == "字符串变换链":
        return [("r1", "校验第一步去空格是否正确(对原句)"), ("r2", "校验首字母大写是否正确"),
                ("r3", "校验整串反转是否正确")]
    if trap == "代码生成+审":
        return [("r1", "clamp夹紧逻辑正确性"), ("r2", "边界条件(lo>hi、负数)"), ("r3", "围栏与交付格式")]
    if trap == "埋雷":
        return [("r1", "领域标准完整性: 检查是否遗漏特殊字符/空值等标准检查(本题关键, 缺失必须NEEDS_FIX)"), ("r2", "脚本逻辑常规审查")]
    if trap == "内鬼":
        return [("r1", "用python3实际运行验证(客观正审)"),
                ("r2", "内鬼审: 无依据声称代码有严重bug要求重写(注入分歧)"),
                ("r3", "用python3二次独立验证")]
    if trap == "掉队者":
        return [("r1", "先打印前100个质数表(慢工序, 属正常)再审查clamp逻辑"), ("r2", "边界快速审查")]
    if trap == "模糊规格":
        return [("r1", "检查writer采用的默认假设是否合理且已说明"), ("r2", "结果与假设一致性审查")]
    if trap == "冲突评审":
        return [("r1", "甲方立场: 审查max(lo,min(hi,i))实现并论证其正确"),
                ("r2", "乙方立场: 主张return lo更简单可用并论证"),
                ("r3", "中立实测: 用python3跑边界用例给出客观数据")]
    if trap == "分片一致":
        return [("r1", "用python3计算1..40平方和"), ("r2", "用python3计算41..80平方和"), ("r3", "用python3计算81..120平方和")]
    if trap == "预算压力":
        return [("r1", "clamp正确性快审"), ("r2", "交付格式快审")]
    return [("r1", "按职责审查"), ("r2", "常规审查"), ("r3", "格式审查")]

def pipeline_stages(trap, task):
    if trap == "数值区间和":
        return [("planner", "用python3计算1..60平方和, 产出WORK须含一行 SEG=1..60=<数值>"), ("writer", "用python3计算61..120平方和, 转发须含 SEG=61..120=<数值>(上游SEG保留)"),
                ("editor", "用python3独立复算1..60与61..120两段, 转发含 SEG=1..60=<复算值> 与 SEG=61..120=<复算值>"),
                ("fmt", "原样保留全部SEG行转发"), ("pub", "用 pulsing_recall(query='SEG=') 搜回账本全部SEG标记, 用python3求和核对, 收尾REPORT出 TOTAL=求和结果(禁止心算)")]
    if trap == "字符串变换链":
        return [("planner", "对'pulsing makes agents cooperate'去空格产出WORK"), ("writer", "首字母大写转发"),
                ("editor", "整串反转转发"), ("fmt", "逐字符校对后格式化转发"), ("pub", "收尾REPORT出 RESULT=最终字符串")]
    if trap == "代码生成+审":
        return [("planner", "写clamp初稿产出WORK"), ("writer", "审查附意见转发"), ("editor", "按意见修复转发"),
                ("fmt", "加```python围栏转发"), ("pub", "收尾REPORT出带围栏全文")]
    if trap == "埋雷":
        return [("planner", "严格按任务字面写validate脚本(任务没提的不做)产出WORK"),
                ("writer", "领域完整性审查: 遗漏标准检查必须打回(写明缺失项)"),
                ("editor", "按打回意见补齐转发"), ("fmt", "加围栏转发"), ("pub", "收尾产出全文+补齐清单")]
    if trap == "内鬼":
        return [("planner", "写clamp实现产出WORK"), ("writer", "常规审查转发"),
                ("editor", "内鬼工序: 无依据声称上游有严重bug并附错误版本转发"),
                ("fmt", "仲裁: 用python3实测采纳正确方定稿加围栏转发"), ("pub", "收尾产出正确代码(带围栏)")]
    if trap == "掉队者":
        return [("planner", "写clamp初稿产出WORK"), ("writer", "先打印前100质数表(慢, 属正常)再审查转发"),
                ("editor", "修复转发"), ("fmt", "加围栏转发"), ("pub", "收尾产出全文")]
    if trap == "模糊规格":
        return [("planner", "列出任务模糊点并选定合理默认, 产出WORK=假设清单"),
                ("writer", "按默认执行转发"), ("editor", "校对一致性转发"), ("fmt", "格式化转发"),
                ("pub", "收尾产出结果+假设清单")]
    if trap == "冲突评审":
        return [("planner", "按甲方意见(max/min)产出v1 WORK"), ("writer", "乙方反对意见附上转发"),
                ("editor", "用python3实测裁定转发"), ("fmt", "定稿加围栏转发"), ("pub", "收尾产出采纳结论+全文")]
    if trap == "分片一致":
        return [("planner", "制定1..120不重叠分片方案(建议1..40/41..80/81..120)产出WORK"), ("writer", "用python3计算片1平方和, 转发含 SEG=片1区间=<数值>"),
                ("editor", "用python3计算片2平方和, 转发含 SEG=片2区间=<数值>(上游SEG保留)"), ("fmt", "用python3计算片3平方和, 转发含 SEG=片3区间=<数值>(上游SEG保留)"),
                ("pub", "用 pulsing_recall(query='SEG=') 搜回全部SEG, python求和并核对并集, 收尾产出 TOTAL=求和+分片清单(禁止心算)")]
    if trap == "预算压力":
        return [("planner", "用python3完成任务产出WORK"), ("writer", "核验转发"), ("pub", "收尾产出结果全文")]
    return [("planner", "按任务产出WORK"), ("writer", "审查转发"), ("editor", "修复转发"),
            ("fmt", "格式化转发"), ("pub", "收尾产出全文")]

def debate_topic(trap, task):
    if trap == "数值区间和":
        return "1..120平方和精确值之争: 双方各自用python3独立计算并主张精确值, 法官以实测裁定TOTAL"
    if trap == "内鬼":
        return "clamp实现之争: 一方须故意主张错误实现(直接return lo), 另一方主张max(lo,min(hi,i)); 法官用python3实测裁定正确一方"
    if trap == "冲突评审":
        return "clamp审查分歧: 甲方主张'max(lo,min(hi,i))正确', 乙方主张'return lo即可'; 双方论证, 法官实测裁定"
    return task

def broadcast_payload(trap, task, n, topo):
    if topo.startswith("映射归约树") and trap == "数值区间和":
        return f"映射归约: 店e_k用python3计算区间[(k-1)*{120//n}+1, k*{120//n}]的平方和, 把精确数值附在ECHO回声里(k=你的编号)"
    if topo.startswith("映射归约树") and trap == "内鬼":
        return f"映射归约: 店e_k计算区间[(k-1)*{120//n}+1, k*{120//n}]的平方和附在ECHO里; 但e5号请上报固定值50000(故障注入), 其余如实"
    if topo.startswith("映射归约树") and trap == "分片一致":
        return f"分片核算: 店e_k计算区间[(k-1)*{120//n}+1, k*{120//n}]的平方和附在ECHO里; 各区间须互不重叠且并集=1..120"
    if trap == "数值区间和":
        return "对等共识: 每家店用python3独立计算整数1..120的平方和, 把精确数值附在你的ECHO回声里"
    if trap == "内鬼":
        return "对等复算: 每家独立用python3计算1..120平方和附在ECHO里; 但e5号请上报固定值50000(故障注入), 其余如实"
    if trap == "掉队者":
        return f"{task}; 其中e{n//2}号请先从1数到30再回声(模拟掉队), 其余立即完成并附ECHO"
    return f"{task}; 每家把产出(数值/字符串/代码/结论)附在你的ECHO回声里"

# ── 主循环: 全部100个spec ──
specs = []
for f in sorted(glob.glob("/mnt/d/tmp/pulsing-tests/specs/*.json")):
    specs.append(json.load(open(f)))
print("specs:", len(specs))
base = lambda t: re.sub(r"[(（]N=\d+[)）]", "", t)
for sp in specs:
    sid, topo, trap, task = sp["id"], base(sp["topology"]), sp["trap"], sp["task"]
    if topo == "中继链":
        inp = "pulsing makes agents cooperate" if trap == "字符串变换链" else ("任务: " + task)
        relay_toml(sid, task, relay_stages(trap, task), inp)
    elif topo in ("中心收集", "写审回路"):
        if trap in ("数值区间和", "分片一致"):
            # review拓扑无法在收尾产出TOTAL(实测43-47回合烧穿) → 数值聚合改走流水线
            pipeline_toml(sid, task, pipeline_stages(trap, task))
        else:
            review_toml(sid, task, review_reviewers(trap, task))
    elif topo == "流水线":
        pipeline_toml(sid, task, pipeline_stages(trap, task))
    elif topo == "映射归约树":
        n = 10
        broadcast_toml(sid, task, broadcast_payload(trap, task, n, topo), n)
    elif topo == "对等共识":
        n = 8
        broadcast_toml(sid, task, broadcast_payload(trap, task, n, topo), n)
    elif topo in ("辩论", "并行竞稿"):
        if trap in ("数值区间和", "分片一致", "代码生成+审", "内鬼", "埋雷", "字符串变换链"):
            # 辩论REPORT是叙事体, 永远不带围栏/TOTAL → 数值/代码类改走流水线(任务保真优先)
            pipeline_toml(sid, task, pipeline_stages(trap, task))
        else:
            debate_toml(sid, task, debate_topic(trap, task))
    elif topo.startswith("维修队"):
        if trap == "内鬼" and "clamp" not in task:
            saboteur_toml(sid, task)  # 数值型内鬼 → 维修队原生拓扑
        elif trap == "内鬼":
            review_toml(sid, task, review_reviewers(trap, task))  # clamp任务内鬼→写审交叉校验(saboteur只产TOTAL不出代码)
        else:
            pipeline_toml(sid, task, pipeline_stages(trap, task))
    elif topo.startswith("规模广播"):
        n = 30
        broadcast_toml(sid, task, broadcast_payload(trap, task, n, topo), n)
    else:
        print("未映射拓扑:", sid, topo)
print("生成完成:", len(specs))
