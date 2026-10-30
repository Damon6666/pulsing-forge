# 场景注册表（Phase 2 规范草案）

场景是数据，分享成本极低——注册表就是网络效应的落点。

## 目录布局

```
scenarios/
├── registry.toml                  # 索引: 元数据+兼容性声明
├── leak-review.toml               # 出厂场景
├── security-review.toml
└── syntax-review.toml
```

## registry.toml 格式

```toml
[[scenario]]
id = "leak-review"
name = "sklearn 数据泄漏审查"
version = "1.0.0"
tags = ["code-review", "ml"]
adapters = ["builtin", "codex"]    # 已验证可跑的店员层
min_agents = 6
max_agents = 6
avg_llm_calls = 35                  # builtin 模式的回合数(成本预估)
license = "Apache-2.0"
```

## 分享协议

1. 场景文件须通过 `forge run` 的 CI 门（确定性场景必须 PASS）
2. 场景不得含 API key、内部 URL 或任何私密
3. tag 遵循受控词表（`code-review`/`ml`/`consensus`/`pipeline`/`debate`/`scale`）
4. 版本语义化；场景变更需附 changelog 行
