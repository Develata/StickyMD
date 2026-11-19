# 02_positioning_and_scope.md - 产品定位与边界

## Metadata

- `Layer`: Foundation
- `Status`: Approved Contract
- `Version`: 0.5.0
- `Last Review`: 2026-11-19
- `Scope`: StickyMD 的存在理由、本体、便签模型、平台与 v1 范围、Non-Goals、优先级与功能拒绝标准

---

## Purpose

StickyMD 的存在理由只有一个：

> 提供一张极度轻量、原生、常驻 Windows 11 桌面的 Markdown 草稿纸：
> 打开即写，自动保存，公式可靠，贴边即隐，需要时迅速出现。

它服务于“临时性的、单张的、以文字和数学公式为主的思考草稿”，
不服务于知识沉淀、文档组织或长期内容管理。

---

## Boundary

本章只固定产品本体、平台、v1 能力边界、优先级与拒绝规则；不选择具体库、不定义
运行时状态字段、不规定 UI 控件实现。后续章节只能细化这些事实，不能扩大本章 scope。

## Owned Objects

本章定义 Note、Document Text、Preview、Managed Image Asset、Runtime Config、
Window Placement、Editor Session 的本体边界，但不拥有任何运行时实例。

## Inputs

USER 冻结的 v1 产品事实与工程宪法。

## Outputs

供后续 plan、feature、acceptance 与实现使用的产品边界和取舍顺序。

## State Changes

Not applicable。产品定位章节不执行运行时状态变化。

## Configuration

Not applicable。本章不定义可配置项；冻结默认值由对应能力章节持有。

## Lifecycle

本合同在整个 v1 生命周期内有效；骨架级修改必须经过 USER 批准。

## Extension / Replacement Points

Not applicable。新能力只有通过本章的本体必要性、同域延伸性与异质侵入性审查后，
才可能进入后续 plan。

## Performance Critical Paths

Not applicable。本章只规定性能不得压过正确性、IME 与数据安全。

---

## 系统本体

StickyMD 的本体是：

> 一个极致优化、常驻 Windows 11 桌面的、以 Rust 为主体实现的、近乎零负担的
> Markdown 临时草稿纸。

核心本体对象只有：

```text
Note
Document Text
Preview
Managed Image Asset
Runtime Config
Window Placement
Editor Session
```

本体对象不是“首版功能清单”。首版功能只决定实现顺序，不定义系统本体
（宪法 3.8 本体优先规则）。

---

## 一张便签模型

- 首版永远只编辑一个 canonical working note：`<note-root>/note/note.md`（Note Root 见 `01`）。
- **一个 Note Root 就是一张便签的身份边界。** 便携版的 Note Root 是程序目录；macOS 安装版的 Note Root 由 app 的名字决定（下一节）。
- 不提供：新建文档、打开文档、最近文件、文件树、多标签页、搜索全部笔记。
- 需要第二张便签时，便携版复制整个程序目录到别处；macOS 安装版用另一个名字的 app 副本。
- 同一 Note Root 只能运行一个实例（第二个实例唤醒第一个并退出）；
  不同 Note Root 的实例完全独立，可同时运行。

<a id="portable-and-installed"></a>
## 便携版与安装版

- **Windows：纯 portable。** 解压即用。无 MSI / MSIX / Store 安装。所有数据只存在于程序目录内：
  `note/note.md`、`note/config.toml`、`note/images/`、`note/.trash/`。
- **macOS：位置决定便签放在哪里，名字决定是哪一张**（USER 批准 2026-11-19，[安装版报告](../report/2026-11-19-macos-installed-note-location-r2.md)）。
  - **安装版：** `StickyMD.app` 直接放在 `/Applications` 或 `<家目录>/Applications` 里时，便签在
    `<家目录>/Library/Application Support/StickyMD/<键>/note/`，键是 bundle 目录名去掉 `.app` 后的 NFC 形式。拖进“应用程序”就是安装，
    没有安装器，也不写系统设置。
  - **便携版：** 其他任何位置。便签在 `.app` 旁边的 `note/`，名字不起作用。
  - 判定比较目录的文件系统身份，不比较路径字符串；判定规则见 `05` #note-root。
  - 便携版与安装版之间不自动迁移，README 写明手工迁移步骤；目标已有 `note/` 时就停下。
- Note Root 不可写时，显示：

  > 当前目录不可写，请将程序移动到有写权限的文件夹。

  然后退出。macOS 保留“当前目录不可写”这个前缀，后半句按情形给出移动建议（`05` #startup-sequence）。
- **禁止按失败回退。** 位置判定、目录创建、加锁或写入任一步失败，都提示后退出，绝不因为失败换到另一个位置：
  Windows 不使用 AppData、LocalAppData、Documents、Registry 或任何用户配置目录；macOS 的模式只由 `.app` 的位置决定，
  安装版位于 `Application Support` 是位置规则的结果，不是 fallback。按失败回退会破坏“Note Root 就是便签身份”的本体模型。

<a id="platforms"></a>
## 平台

```text
交付形态：Windows 11 x64（portable ZIP）；macOS 27+ arm64（ad-hoc 签名的 ZIP，下一版本起与 Windows 同版公开发布）
共享实现：面向 Windows、Debian、macOS
```

不支持 Windows 10、Windows ARM64、Linux 交付形态、Intel Mac。共享实现（Shared Application、Shared Desktop、
`stickymd-core`、`stickymd-render`）面向 Windows、Debian、macOS 集中维护，平台依赖隔离在 Platform Host 的
adapter 之后（`03` 共享实现节、`09_windows_shell.md`）。USER 决定（2026-10-31）：先完成共享实现并接回 Windows 壳，
另外两端在对应平台实现。

“共享实现面向三端”不等于“已交付三端产品”。Debian 的桌面环境与 X11/Wayland 支持矩阵、便签目录与实例身份、缺少贴边定位、
隐藏或置顶能力时的产品行为、默认字体、输入法验收矩阵、资源与性能门，在建立 Debian 宿主前须另行批准，本节不替其定案；
在此之前 Debian 的一切验收保持 `NOT TESTED`。

<a id="macos-host"></a>
### macOS 宿主

USER 批准（2026-11-10，[决定报告](../report/2026-11-09-macos-platform-decision.md)）。

| 项 | 合同 |
| --- | --- |
| 版本与架构 | macOS 27 及以上，仅 `aarch64-apple-darwin`；不打 x86_64 或 universal 包 |
| 应用身份 | `StickyMD.app`，`io.github.develata.stickymd`，`LSUIElement`，无 Dock 图标，不启用沙盒 |
| 签名与分发 | ad-hoc 签名，不购买 Developer ID，不公证（USER 决定 2026-11-19，[分发报告](../report/2026-11-19-macos-public-distribution-decision-r2.md)）。GitHub Release 附 ad-hoc 签名的 ZIP，与 Windows 同版发布（`11` #release-attachment-set）；用户第一次打开时须在“系统设置 → 隐私与安全性”中允许（推断，由 Phase 18 的下载路径验收确认） |
| 便签目录 | 便携版：包含 `StickyMD.app` 的目录是 Program Directory 与 Note Root，数据在 `<dir>/note/`，不进 bundle；复制整个目录得到第二张便签。安装版：`.app` 直接位于 `/Applications` 或 `<家目录>/Applications` 时，Note Root 为 `<家目录>/Library/Application Support/StickyMD/<键>/`（`05` #note-root） |
| 卷要求 | 卷须支持不覆盖改名（`RENAME_EXCL`），否则提示后退出（`05` macOS 节） |
| 默认字体 | 中文 PingFang SC；西文 Times New Roman → Georgia；代码 Menlo；数学不变 |
| 快捷键 | Cmd 替代 Ctrl（`07` macOS 节） |
| 窗口行为 | `09` macOS 实现映射；多屏保持 `NOT TESTED` |

未在对应环境运行的 macOS 验收项保持 `NOT TESTED`。v1 Scope 中的字体、输入法、托盘等条目以 Windows 为描述对象，macOS 的对应差异以本节及所引章节为准。

---

## v1 Scope（已冻结）

以下能力均为 USER 已批准的骨架级决策，忠实建模即可，不再讨论取舍：

1. 三视图：Source / Preview / Split（固定 50/50，分隔线不可拖动）；Split 以 generation-bound source range 做默认开启、可关闭的语义滚动同步，并保留两侧独立滚动位置。
2. Preview debounce 1000 ms；切换纯 Preview 时立即刷新；stale generation 丢弃。
3. Markdown 方言：CommonMark + GFM + Comrak math delimiter（语义归 Comrak）。
4. 数学：RaTeX / KaTeX-compatible；四种 delimiter；错误公式显示原文 + 轻提示，不崩溃。
5. Raw HTML：不执行，按 literal/code 风格展示原文。
6. 字体：中文仿宋_GB2312 系、Latin Times New Roman、代码 Consolas、数学 RaTeX 字体；字符/脚本级 font run。
7. IME：微软拼音、微信输入法为一级验收；纯 Rust 路线优先，RichEdit 仅为受控 fallback。
8. Autosave debounce 约 650 ms；失焦/退出/Ctrl+S 立即保存；原子写入。
9. Undo/Redo：仅当前进程，max 256 entries 或 4 MiB，先到先淘汰。
10. 图片粘贴：managed 命名 `stickymd-<hash>.<ext>`；保留原编码；截图转 PNG；GC 只作用于 managed。
11. 导出（Ctrl+Shift+S）：复制引用图片到 `<name>-assets/`，重写相对路径；不是“另存为”。
12. 窗口：Always on top、左/右/上 dock、auto-hide、hover reveal、手动/Esc 收起、opacity 40–100、Light/System/Dark 主题、托盘生命周期、多显示器（混合 DPI、拔插、负坐标）。
13. 文件可靠性：UTF-8（兼容 BOM 读取）、换行风格保留、temp 恢复、外部修改 reconcile、脏冲突 banner、无效 UTF-8 安全处理。
14. Phase 10 交互修订：Windows 传统剪贴板快捷键；Source/Preview/Split 共享 50–300% Content Zoom；220×120 DIP 最小窗口；主窗口采用可聚焦的 Tool Window 身份；24 DIP 最近合格边缘 dock 捕获。
15. Phase 14 USER 批准的候选冻结前修订：当前 Source 文档纯文本查找/替换（大小写开关、无正则）；数学分隔符转换控件使用紧凑、清晰的 `$` 标识。
16. Phase 17（USER 批准 2026-11-14；下一版本实现，此前发布的版本不含）：复制按“起止点所见即所得”统一（`#copy-what-you-see`、`06` #preview-copy-semantics）；代码块与块级公式的复制侧条（`06` #preview-copy-rail）；提示分级与统一提示条（`04` #notice-state、`09` #notices）；Preview 文末留白（`09` #vertical-scrollbars）。Phase 17 落地前，现行条款继续有效；各章 Phase 17 小节写明它取代哪些条款。

详细契约见 `docs/plan/05..11`；用户可见行为投影见 `docs/features/`；
验收见 `docs/acceptance-cases/`。

---

## Non-Goals（v1 明确拒绝）

### 文档管理

New / Open / Recent files、多文档、多标签页、文件树、Workspace、Vault、数据库、
全局搜索、标签、双向链接、Backlink、Graph view。

### 云与账户

登录、云同步、WebDAV、Git 同步 UI、OneDrive/Dropbox 集成、账户系统、协同编辑。

### 编辑器膨胀

WYSIWYG、Typora 模式、Vim/Emacs 模式、多光标、插件系统、宏、命令面板、LSP、
语法高亮、代码执行、Terminal、Mermaid、PlantUML、LaTeX 文档编译、PDF/HTML 导出。

### AI 与网络

AI 写作/总结/补全、遥测、Analytics、崩溃自动上传、远程图片下载、自动更新、广告、
任何在线服务。程序默认无网络依赖。

### 视觉扩展

自定义 CSS、主题市场、背景图片、Acrylic/Mica/毛玻璃、动态壁纸、可配置动画/圆角/阴影、
可拖动 split 分隔线。

### 系统扩展

MSI/MSIX、Microsoft Store、Windows 10、Windows ARM64 v1、Linux app v1、Intel Mac、
Mac App Store、自动开机启动 v1、全局快捷键 v1。

---

## 优先级（继承宪法 1.4）

目标冲突时的严格取舍顺序：

```text
正确性 / 功能实现
> 可用性 / 使用体验
> 根基兼容性
> 可维护性 / 可诊断性
> 性能
> 内存占用
> 外存占用
> 其它次级因素
```

### 在 StickyMD 语境下的解释

低内存是重要目标，但它是**第六优先级**，不是最高目标：

- 不能为了省 3 MB 内存破坏 IME 正确性。
- 不能为了减少代码量破坏原子保存。
- 不能为了性能绕过冲突模型或吞掉保存错误。
- 不能为了启动速度跳过可写性检查或单实例检查。

性能与内存的量化目标见 `10_performance_reliability.md`，其定位是
Initial Engineering Targets，未经实测不得对外宣传。

### “极简”是什么意思

“极简”指**功能面窄、负担近零**：

- 只有一张纸、一个窗口、一个托盘。
- 没有设置页面、没有主题市场、没有引导流程。
- 空闲时几乎不占 CPU，内存有硬上限。
- 每个新增能力都必须通过宪法 3.13 的三层判定，否则拒绝。

“极简”**不是**指实现粗糙：数据安全、原子写入、IME 正确性、错误路径一个都不能省。

<a id="copy-what-you-see"></a>
### 复制：起止点所见即所得（Phase 17，USER 批准 2026-11-14）

复制的范围由用户看到的起点与终点决定。Source、Preview 与复制侧条遵循下面这条用户规则，取文本的路径各自实现：

- **Source：** 符号本来就看得见，按原样切片（`07`）。
- **Preview：** 符号不可见，按 `06` #preview-copy-semantics 把选区映射回源码，并处理被切开的结构。

- **起止点都落在同一个公式或代码块的内容里：只复制内容**，不带分隔符或围栏。要复制一个公式或一段代码，用户要的是里面的东西；
  不带包装的内容也是各种粘贴目标（LaTeX 文档、公式编辑器、其他 Markdown 工具）都能接受的最小公分母，怎么包装由目标决定。
- **范围超出单个对象：保留 Markdown 渲染符号。** 这些符号是通用的，粘贴到 Markdown 环境中多数仍能被渲染；去掉它们，
  只会留下只能用肉眼读的文字。
- **诚实的边界：**
  - 在 Preview 中，选区一端在粗体、链接等行内结构里、另一端在外时，复制整个结构，会比高亮多出几个字；
  - 引用式链接、表格片段等结构单独粘贴后不一定还能渲染。

所见即所得指起止点，而不是 Preview 的像素。这顺应“复制一个东西就得到那个东西”的习惯，省去额外的心智负担：
用户只需记住一条规则，两种视图各自按自己看得见的内容执行它。

---

## Failure Paths

- Note Root 不可写：明确提示后退出（不静默 fallback）。
- 功能请求落在 Non-Goals：默认拒绝 PR，引用本章。
- 目录身份解析失败（路径无法 canonical 化，或 macOS 的位置判定、键或家目录无法确定）：按启动失败处理，提示原因并退出。

## Verification

- 本章内容通过 `docs/coverage-matrix.md` 与 features/acceptance 对照。
- 任何新功能请求必须先比对本章 v1 Scope 与 Non-Goals。

## Non-Goals

Not applicable（本章本身即 Non-Goals 的载体）。
