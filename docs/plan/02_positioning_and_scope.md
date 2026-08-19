# 02_positioning_and_scope.md - 产品定位与边界

## Metadata

- `Layer`: Foundation
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-19
- `Scope`: StickyMD 的存在理由、本体、便签模型、平台与 v1 范围、Non-Goals、优先级与功能拒绝标准

---

## Purpose

StickyMD 的存在理由只有一个：

> 提供一张极度轻量、原生、常驻 Windows 11 桌面的 Markdown 草稿纸：
> 打开即写，自动保存，公式可靠，贴边即隐，需要时迅速出现。

它服务于“临时性的、单张的、以文字和数学公式为主的思考草稿”，
不服务于知识沉淀、文档组织或长期内容管理。

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

- 首版永远只编辑一个 canonical working note：`<program-dir>/note/note.md`。
- **一个程序目录就是一张便签的身份边界。**
- 不提供：新建文档、打开文档、最近文件、文件树、多标签页、搜索全部笔记。
- 需要第二张便签时，用户复制整个程序目录到别处。
- 同一程序目录只能运行一个实例（第二个实例唤醒第一个并退出）；
  不同目录的实例完全独立，可同时运行。

## Portable 目录身份

- 纯 portable：解压即用。无 MSI / MSIX / Store 安装。
- 所有数据只存在于程序目录内：`note/note.md`、`note/config.toml`、`note/images/`、`note/.trash/`。
- 程序目录不可写时，显示：

  > 当前目录不可写，请将程序移动到有写权限的文件夹。

  然后退出。
- **禁止** fallback 到 AppData、LocalAppData、Documents、Registry 或任何用户配置目录。
  这类 fallback 会破坏“目录就是便签身份”的本体模型。

## 平台

```text
Windows 11 x64 only
```

v1 不支持 Windows 10、Linux、macOS、ARM64。
但平台依赖必须被隔离在平台 adapter 之后，为未来替代留出干净边界
（见 `09_windows_shell.md`）。

---

## v1 Scope（已冻结）

以下能力均为 USER 已批准的骨架级决策，忠实建模即可，不再讨论取舍：

1. 三视图：Source / Preview / Split（固定 50/50，分隔线不可拖动）。
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
12. 窗口：Always on top、左/右/上 dock、auto-hide、hover reveal、手动/Esc 收起、opacity 70–100、Light/System/Dark 主题、托盘生命周期、多显示器（混合 DPI、拔插、负坐标）。
13. 文件可靠性：UTF-8（兼容 BOM 读取）、换行风格保留、temp 恢复、外部修改 reconcile、脏冲突 banner、无效 UTF-8 安全处理。

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

MSI/MSIX、Microsoft Store、Windows 10、ARM64 v1、Linux/macOS app v1、
自动开机启动 v1、全局快捷键 v1。

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

---

## Failure Paths（本章适用部分）

- 程序目录不可写：明确提示后退出（不静默 fallback）。
- 功能请求落在 Non-Goals：默认拒绝 PR，引用本章。
- 目录身份解析失败（路径无法 canonical 化）：按启动失败处理，记录并退出。

## Verification

- 本章内容通过 `docs/coverage-matrix.md` 与 features/acceptance 对照。
- 任何新功能请求必须先比对本章 v1 Scope 与 Non-Goals。

## Non-Goals

Not applicable（本章本身即 Non-Goals 的载体）。
