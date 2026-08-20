# docs/plan/AGENTS.md — 工程合同目录规则

`docs/plan/` 是 StickyMD 工程骨架与工程合同的**唯一权威文档树**。

```text
docs/plan → projection docs → code
```

代码与投影文档（features / acceptance / overview）都不得反向修改这里的契约。
修改本目录中任何骨架级内容，必须先提交 `docs/report/` 分析报告并获得 USER 批准。

---

## 章节清单

| 文件 | 主题 | Layer |
| --- | --- | --- |
| `00_engineering_constitution.md` | 工程宪法（USER 原文） | Foundation |
| `01_terminology.md` | 术语固定 | Foundation |
| `02_positioning_and_scope.md` | 产品定位与边界 | Foundation |
| `03_system_architecture.md` | 四层架构 + Object Plane | Architecture |
| `04_runtime_state_model.md` | 运行时状态与权威模型 | Architecture |
| `05_document_persistence.md` | 文档持久化合同 | Runtime |
| `06_markdown_math_rendering.md` | Markdown / 数学 / 预览合同 | Capability |
| `07_editor_and_ime.md` | 源码编辑器与输入法合同 | Capability |
| `08_assets_and_export.md` | 图片资产与导出合同 | Capability |
| `09_windows_shell.md` | Windows 壳层与平台边界合同 | Capability |
| `10_performance_reliability.md` | 性能与可靠性合同 | Verification |
| `11_testing_and_release.md` | 测试与发布合同 | Verification |

---

## 章节格式

每章顶部必须包含统一 Metadata 块：

```markdown
# <filename> - <title>

## Metadata

- `Layer`: Foundation | Architecture | Runtime | Capability | Verification
- `Status`: Governing Rule | Approved Contract | Draft
- `Version`: 0.1.0
- `Last Review`: <YYYY-MM-DD>
- `Scope`: ...
```

除工程宪法原文（`00`）与术语表（`01`）外，每章必须用同名二级标题回答以下各项：

```text
Purpose            目的
Boundary           边界（管什么、不管什么）
Owned Objects      拥有的 Object Plane 对象
Inputs             输入
Outputs            输出
State Changes      状态变化
Failure Paths      失败路径（一级内容，不得省略）
Configuration      配置入口
Lifecycle          生命周期
Extension / Replacement Points   扩展点 / 替换点
Performance Critical Paths       性能关键路径
Verification       验证方式
Non-Goals          明确不做的事
```

`00` 只能在 USER 宪法原文之前增加 Metadata；`01` 按每个术语的
Definition / Authority / Not equivalent to / Lifetime 固定格式组织。其余章节若某项不适用，
必须显式写：

```text
Not applicable
```

而不是省略，避免语义不明确。

---

## Stable Anchors

`plan_ref` 指向章节内 stable anchor。所有被代码引用的 anchor 必须使用显式、纯 ASCII ID，
不能依赖 Markdown renderer 对中文、标点或大小写的自动 slug 规则：

```markdown
<a id="atomic-save"></a>
## Atomic Save
```

引用写作 `docs/plan/05_document_persistence.md#atomic-save`。

重命名或删除已被 `plan_ref` 引用的 anchor，视为契约变更，需要走审批流程。

---

## 本目录禁令

- 不得在本目录写入产品营销文案或教程（属于 README / overview）。
- 不得在本目录记录一次性分析（属于 `docs/report/`）。
- 不得在本目录记录决策历史叙事（属于 `docs/adr/`）。
- 不得引入与 `00_engineering_constitution.md` 冲突的条款；宪法优先级最高。
