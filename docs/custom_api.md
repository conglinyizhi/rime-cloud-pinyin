# 自定义 API 文档

## 概述

cloud_pinyin 支持自定义云拼音 API。当你有自建的拼音输入法后端时，只需让它返回指定格式的 JSON，即可接入。

## 请求

### URL 模板

在 `rime.lua` 中通过 `api_url` 配置，使用 `{input}` 作为拼音占位符：

```lua
cloud_pinyin_cli.config.engine = "custom"
cloud_pinyin_cli.config.api_url = "https://your-api.com/pinyin?input={input}"
```

`{input}` 会被替换为经过 URL 编码的拼音字符串（双拼模式下已转换为全拼）。

### 请求方式

- **方法**: GET
- **超时**: 5 秒

### 示例

用户输入拼音 `ni'hao`，配置为 `https://your-api.com/pinyin?input={input}`，实际请求：

```
GET https://your-api.com/pinyin?input=ni%27hao
```

## 响应

### 格式

返回 JSON，顶层 `candidates` 为候选数组，最多取前 5 个。

```json
{
  "candidates": [
    {
      "text": "你好",
      "preedit": "ni hao"
    },
    {
      "text": "你号"
    }
  ]
}
```

### 字段说明

| 字段 | 层级 | 类型 | 必填 | 说明 |
|------|------|------|------|------|
| `candidates` | 顶层 | array | 是 | 候选列表 |
| `text` | candidates 元素 | string | 是 | 候选词文本，为空时跳过该条 |
| `preedit` | candidates 元素 | string | 否 | 预编辑文本（如全拼注释），不提供时双拼模式下自动由本地转换生成 |

### 响应示例

```json
{
  "candidates": [
    { "text": "你好", "preedit": "ni hao" },
    { "text": "你号", "preedit": "ni hao" },
    { "text": "泥好", "preedit": "ni hao" },
    { "text": "尼豪", "preedit": "ni hao" },
    { "text": "拟好", "preedit": "ni hao" }
  ]
}
```

不提供 `preedit` 的最小示例：

```json
{
  "candidates": [
    { "text": "你好" },
    { "text": "你号" }
  ]
}
```

## CLI 使用

```bash
cloud_pinyin --engine custom --api-url "https://your-api.com/pinyin?input={input}" --format tsv "nihao"
```

`--format tsv` 输出格式：

```
你好	5	ni hao
你号	5	ni hao
```

## 错误处理

- API 返回非 JSON 或网络错误时，静默忽略，不显示候选
- `candidates` 不存在或为空数组时，不显示候选
- 单条候选 `text` 为空字符串时跳过该条
- 超时 5 秒后放弃请求
