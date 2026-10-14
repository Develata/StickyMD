# 人工验收复用与产品域拆细：设计提案（2026-10-10）

> 状态：**提案，待 USER 批准**（第 1 轮审查后的修正见文末"修订"一节）。根因 3 修改 plan 11 的人工验收合同，属于骨架级变化；按 AGENTS.md 先报告、
> 经 USER 批准写入 `docs/plan/` 后才实施。本文不是契约。

## 背景与授权

USER 要求"如果一个模块没有被动过，那么对应的测试就不需要再跑一遍"，并在 2026-10-09 批准按四个根因修复；
根因 1、2（U1）已推送为 `fix/qualification-ledger-reuse`（`972aad4..fcf99dc`，九轮审查，末轮 PASS）。
2026-10-10 USER 指示："之后再去把根因3和根因4做好，同样的，还是和gpt交互多轮迭代优化清楚以后再分批
commit + push。"

- 根因 3：24 项人工验收（P12-Mxx 中的 Manual 项）不在账本里，绑定当前候选，换候选必须全部重做。
- 根因 4：产品域划分过粗（`EDITOR`、`PREVIEW` 进入几乎所有模块），复用很少触发。

早先方案审查对人工复用给出的阻断条件：依赖闭包必须有调用链依据（`app/input.rs` 同时分发窗口、Preview 与
滚轮）；环境身份必须完整（含补丁号）；同一指纹后来的 `MANUAL_FAIL` 必须优先；M34 Clean VM 保持精确制品
绑定；保留 Tier C 语义、候选绑定的 waiver 与当前候选汇总。

## 已核实现状

- **合同**（plan 11）：`MANUAL PASS` 指"当前提交上的正式人工矩阵已执行"；"一次性终端命令、未提交脚本、
  主观观察、旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"；"最终 manual PASS 仍额外绑定 Promoted
  Candidate"；waiver 只接受具体 `WAIVER-P12-Mxx`，身份来自同一 Source Freeze。
- **收据**：单文件 `dist/evidence/manual-acceptance.json`，绑定候选的 source/EXE/ZIP/version；续录与 readiness
  共用 `manual_receipt::validated_cases`（U1 第八、九轮）。
- **人工项**（24 项）：Tier A：M01、M02（输入法视觉）、M05（Alt+Tab 后恢复并输入）、M11/M12（左右 dock）、
  M18–M20（220×120 三种模式）、M21（缩放）、M22（透明度）、M23（主题）、M24–M26（Markdown/公式/图片视觉）；
  Tier B：M34（Clean VM）、M35–M37（多屏与拔插）、M38–M40（125/150/200% DPI）；Tier C：M41–M43（睡眠、RDP、
  负坐标显示器）。
- **域映射**（`module_ledger/fingerprint.rs`）：`app/input.rs` 只归 `EDITOR`，但它分发 IME、按键（缩放、剪贴板、
  保存/导出快捷键）、鼠标按键、光标移动与滚轮。现有自动化模块都包含 `EDITOR`，因此这一低估目前没有造成漏判；
  一旦人工项使用更细的域，它就会漏判。
- **发布历史**：v0.1.2→v0.1.3 的产品改动只有 `flow/editor.rs` 与 render crate 的 `lib.rs`、`math_text.rs`；
  v0.1.1→v0.1.2 改了约 44 个产品文件，集中在 Preview 与 source。v0.1.1、v0.1.2 发布时人工项均为 NOT TESTED，
  即目前没有任何可复用的人工 PASS；第一次采用新机制的版本仍需完整人工基线。

## 根因 3：人工项进入账本

### 模型

1. 每个人工项是一个账本模块 `manual-p12-mXX`，复用共享存储、指纹定位、锁与严格记录。evidence 是该项的单条
   观察文档（case、状态、备注、观察时的候选身份、环境身份、记录时间）。
2. **人工项指纹** = 该项声明的产品域输入 + harness 输入（人工记录器与引导会话代码、`phase-12.md` 中该行、
   `docs/plan/11` 与相关 plan 章节）+ **环境身份**。
3. **环境身份**按项声明所需维度，写入指纹：Windows 完整版本（含 UBR 补丁号）、显示器拓扑（数量、排列、每屏
   DPI）、系统主题设置、输入法 profile 与版本（M01/M02/M22/M36 等涉及 IME 的项）。补丁号变化会使声明了 Windows
   版本的项失效；这是审查要求的保守取舍，代价如实接受。
4. **记录最新观察，而非最新成功**：每个 `(项, 指纹)` 只保存最近一次正式观察。`MANUAL_FAIL` 覆盖此前的
   `MANUAL_PASS`，readiness 遇到 FAIL 即阻断；只有之后的 PASS 能再覆盖它。
5. **M34 Clean VM 不复用**：其指纹包含候选 ZIP 的 SHA-256，等价于逐候选重做（它验证的就是这份包在干净机器
   上运行）。
6. **Tier C 与 waiver 不变**：Tier C 在自动化覆盖全部通过时允许 NOT_TESTED；waiver 仍是绑定 Source Freeze 版本
   的 `WAIVER-P12-Mxx`，不进入账本、不跨版本复用。
7. **候选绑定汇总**：readiness 为当前候选写出人工汇总，逐项标明 `RAN PASS`、`REUSED PASS (origin candidate)`、
   `WAIVED`、`NOT TESTED` 或 `FAILED`。发布记录仍绑定当前候选，复用只改变"观察来自哪次运行"。
8. **记录器**：引导会话只列出 `RUN REQUIRED` 的项；每录入一项立即登记到账本（原子发布），中断不丢已录项。

### 域闭包（初始保守）

在根因 4 完成前，人工项的产品域取**全部产品域**，与审查要求一致。这意味着：任何产品代码改动都会使全部人工
项失效；只有"仅工具/文档/版本号"的版本能复用人工结果。v0.1.3 这类改了编辑器代码的版本仍需全部重测。

### plan 11 需要的修改（待批准）

- 将"旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"改为：人工项的正式观察按"项指纹（产品域、harness、
  环境身份）"复用；指纹不变的最近一次正式观察有效，FAIL 优先；M34 逐候选。
- `MANUAL PASS` 的定义从"当前提交上执行"改为"与当前项指纹兼容的最近一次正式观察为 PASS"，并保留 RAN/REUSED
  区分。
- 新增人工项的环境身份维度与采集方式。

## 根因 4：产品域拆细

### 步骤

1. **分类诚实化**（无产品代码改动）：所有分发/路由文件按其实际分发的全部域归类（例如 `app/input.rs` →
   `EDITOR|SHELL|PREVIEW|PERSISTENCE|EXPORT`：鼠标路径到达工具栏、滚动条与窗口（SHELL）、Preview 拖选
（PREVIEW）与 Source（EDITOR），快捷键到达保存与导出）。新增"每个产品文件都必须被显式分类"的守卫测试，
   不再依赖 `ALL_PRODUCT` 兜底。
2. **按域拆分分发**（产品代码重构，行为不变）：把 `app/input.rs`（713 行，已超过 500 行审视线）拆为编辑输入
   （IME、文本键、撤销）、窗口/壳层输入（鼠标拖动、dock 相关）、缩放、快捷键路由；路由层只做分派并归入所有
   目标域，但改动频率低。`preview_input.rs` 已独立。
3. **逐项声明人工项与自动化模块的域**：以调用链为依据，例如 M11/M12（dock）→ `SHELL`；M24（Markdown 视觉）→
   `PREVIEW`；M25 → `PREVIEW|MATH`；M26 → `PREVIEW|IMAGES`；M35/M37 → `SHELL`；涉及输入的项包含 `EDITOR`。

### 预期收益（以 v0.1.3 回放，环境不变且此前有 PASS 为前提）

v0.1.3 的改动落在 `EDITOR`（`flow/editor.rs`）与数学文本转换（`math_text.rs`，拆细后归 `EDITOR|MATH`）。按上面
的声明，M11、M12、M19、M23、M24、M26、M35、M37 约 8 项可复用，其余 16 项仍需重测。没有根因 4，全部 24 项重测。
这一数字依赖逐项域声明，需在审查中逐项核对。

### 风险

- 共享状态耦合：同一进程内不同域通过 `StickyApp` 状态互相影响，路径域只是近似；以调用链与已有自动化覆盖为依据，
  无法证明时归入更宽的域。
- 重构风险：拆分 `input.rs` 改动产品代码，需全部单元测试与 smoke 覆盖，且下一个候选必然重跑全部模块。

## 实施分批（批准后）

1. 根因 4 第 1 步（分类诚实化 + 守卫测试）。
2. 根因 3（人工项账本、环境身份、记录器、readiness 汇总、plan 11 修改）。
3. 根因 4 第 2、3 步（`input.rs` 拆分与逐项域声明）。

每批经 gpt-6-astra 多轮审查后提交并推送。

## 对规格的影响

plan 11 `#module-success-ledger` 与人工验收相关段落、`phase-12.md` 人工项说明、`phase-14.md` 新增验收项、
coverage matrix。

## 修订（第 1 轮审查后，2026-10-10）

gpt-6-astra（max）第 1 轮给出 4 个 BLOCKER、3 个 SHOULD_FIX，逐条对照代码与合同核实后**全部成立**。本节修正
上文；与上文冲突处以本节为准。

### 收益更正（先读）

- **v0.1.3 回放 = 0 项可复用**，不是"约 8 项"。`v0.1.2..972aad4` 除 `flow/editor.rs`、render `lib.rs`、
  `math_text.rs`、`math_text/tests.rs` 外还改了 `docs/plan/07_editor_and_ime.md` 与 `phase-11-b.md`；按现行规则
  （plan 11 `#module-success-ledger`："`docs/plan/`、`docs/acceptance-cases/` 因跨域约束仍对全部功能模块保守传播"，
  实现见 `fingerprint.rs:194`），在任何产品域推理之前全部模块已失效。即使忽略合同变化，render `lib.rs` 归
  `PREVIEW|EDITOR|MATH|IMAGES|EXPORT`，M19/M24/M26 不成立；M23 断言公式、图片与控件，也不成立。
- **拆细后的上限**：只有 M11/M12/M35/M37 有可能，且每项都要先证明 `window_guards`
  （`window_runtime.rs:252`）读到的 composition、搜索、导出、恢复、资产事务、保存状态的写入方都在闭包内；
  按现状这些写入方覆盖大部分产品域，实际可能接近 0。
- **补丁窗口**：环境身份含 UBR，Windows 每次累积更新（每月一次，另有可选预览更新）都使所有声明 Windows 版本的项
  失效。现实的复用场景是：**同一台机器、同一套显示器、同一补丁窗口内，且该版本没有改动该项的任何输入**——
  典型是只改发布流程、release notes、README 或非全局工具代码后重建候选。
- 因此根因 3 的主要价值不是"产品改动后少测"，而是**候选重建时不重测**，以及让下一次完整人工基线以可复用的
  格式落盘。

### 新核实的事实

- plan 11:316"可模块化 manual observation 使用 Last Successful Module Receipt"来自 `748c3d6`（2026-08-30），
  早于 U1，与 :237/:243（旧 commit 收据不能提升人工项）矛盾；"可模块化"在全仓库没有定义。这是现存矛盾。
- 账本合同"记录只允许 `PASSED`"，`01_terminology.md` 的 Last Successful Module Receipt"失败或中止不得覆盖"；
  `record.rs` 只接受 `PASSED`，`store.rs` 每模块保留 8 份、没有条件写入。记录 FAIL 是第二种证据策略，需要新术语。
- waiver：plan 11:258 只接受逐项 key，:652–655 允许 Tier B 组 waiver，代码（`manual_readiness.rs`、
  `decisions.rs`）接受 `WAIVER-TIER-B-v{version}`。这也是现存矛盾，上文"waiver 不变"的现状描述不准确。
- 记录器：逐项记录器（`manual.rs` `record`）覆盖全部 24 项；引导记录器只覆盖 14 个 Tier A 项，且 G2-04
  （M11/M12）、G2-05（M18–M20）一次回答写给多项。
- `windows_build::is_known` 接受三段版本；`ver` 实际输出四段（第四段即 UBR）。
- 工具已有一处 FFI 先例（`atomic_evidence.rs` 的 `MoveFileExW`），没有 COM。
- 产品字体按候选族名顺序取第一个存在的（`fonts.rs`，首选 `仿宋_GB2312`，通常需用户自行安装），因此相同
  Windows 版本不等于相同字体环境。
- plan 11:91 要求"当前与前一个受支持 Windows 11 版本"，现有工具不建模这一要求；这是现存缺口，不由根因 3 解决，
  单独提请注意。

### 修正后的设计

1. **一个注册表、共用存储设施、两种证据策略。** 自动化模块保持 Last Successful Module Receipt。人工项采用新
   术语 **Latest Manual Observation**：每个 `(项, 指纹)` 保存最近一次**完成**的观察，状态只能是 `MANUAL_PASS`
   或 `MANUAL_FAIL`；独立记录类型与校验；readiness 对人工项走人工策略，不进入"所有模块必须 PASS"的通用分支。
2. **生命周期。** `NOT_TESTED`、跳过、中止从不写入账本，也从不覆盖已有记录。`MANUAL_FAIL` 不参与清理（PASS
   仍按每项 8 份清理），直到同一指纹上一次完整 PASS 替换它。FAIL 优先于 waiver 与 Tier C 的非阻断规则。
   写入用条件更新：记录器把展示给观察者的当前记录版本（evidence 摘要或"无"）随提交带回，在写锁内比较，不一致
   即拒绝并要求重新观察；不用时间戳决定先后。写入失败使命令失败，不输出汇总。提供对已有 PASS 显式重测的入口，
   用于登记后来发现的 FAIL。引导会话按项提交，一步覆盖多项时逐项询问状态。
3. **指纹。** 继承现有共享/GLOBAL 规则（manifest、lock、toolchain、registry、指纹算法、证据校验器等），版本号
   规范化沿用现有窄规则；再加该项产品域、harness（记录器、引导步骤、场景表、环境 schema）、合同输入与环境身份。
   上文"只有仅工具/文档/版本号的版本能复用"改为"**不影响该项输入**的改动可复用"。
4. **环境分三类，每项逐一声明。**
   - **主机事实**（readiness 时与当前主机比较，工具经子进程或文件读取自动采集，不新增 FFI）：Windows 四段版本
     （`ver`，含 UBR；人工与 exact evidence 一并收紧为四段，`windows_build.rs` 属全局输入）、系统与应用主题
     （`reg query`）、字体目录清单摘要（系统与用户字体目录的文件名+大小，不读字体字节）。采集失败或 `UNKNOWN`
     即不可复用。
   - **场景参数**（逐场景记录，只检查是否覆盖该项要求的全部场景，不与当前主机比较）：DPI 档位、显示器拓扑、
     主题模式与运行时切换、M21 的缩放档位×视图。由观察者声明并标注来源为"声明"。场景表放在工具内，按
     plan 07/11 与 `phase-12.md` 逐项列出（例如 M01：微软拼音 × 100/150/200%；M23：Light、Dark、System、
     运行时切换）；任一场景 FAIL 则该项 FAIL，缺场景则不能登记 PASS。
   - **工具无法可靠识别的外部依赖 → 不复用，绑定精确制品**：第三方输入法会静默自动更新，TSF profile 不含产品
     版本，微软拼音还有"使用以前版本"开关，相同 profile 名不能证明行为相同。因此断言 IME 行为的项——M01、M02、
     M05、M18、M22、M36、M41——与 M34 一样每份新 ZIP 重测。以后若加入可靠的 IME 采集再放开。
5. **M34** 绑定 ZIP 的 SHA-256 与声明的 Clean VM 基线：同一份字节重新晋升可复用；新 ZIP 即使 EXE 相同也重测。
6. **waiver**：保留现有 Tier B 组 waiver，解析为具体的 Tier B 项集合；把 plan 11:258 改为与 :652–655 一致
   （逐项 key 或绑定版本与 Source Freeze 的 Tier B 组 key，不接受跨 tier 或全局 blanket）。waiver 不写成 PASS，
   不能越过已知 FAIL，不跨版本继承。
7. **候选汇总是派生结果，不是第二份权威。** 每次 readiness 重新读取最新记录；汇总列出完整候选身份，以及逐项
   指纹、场景覆盖、观察记录摘要、origin 与 waiver 引用。旧格式人工收据缺少环境与场景字段，不自动导入。
8. **根因 4 的域证据。** `app/input.rs` 的诚实分类还要包括 `MATH`（工具栏 `ConvertMath`）与 `IMAGES|ASSETS`
   （粘贴），即接近全部产品域。缩窄必须给出"入口 → 调用/回调/Effect → 共享状态读写 → 验收断言"的依据，共享
   状态的写入方进入闭包；无法证明时保持宽域。显式分类守卫是附加检查，运行时遇到未分类文件仍保守回退到全部
   产品域。拆文件只改善职责边界，不能当作解耦证明。

### plan 修改范围（整条人工证据链）

- plan 11：:237 `MANUAL PASS` 定义（与当前项指纹兼容、场景完整、最新完成观察为 PASS，并区分 RAN/REUSED）；
  :243 旧收据规则；:258–259 waiver；:316 把"可模块化 manual observation"改为指向人工策略；
  `#module-success-ledger` 增加人工策略（记录 FAIL、FAIL 不清理、条件写入、NOT_TESTED 不写入）；:336–345
  记录器与 Windows 四段版本；环境三分类与 IME 项、M34 的绑定规则。
- `01_terminology.md`：新增 Latest Manual Observation。
- 投影：`phase-12.md` 人工项说明、`phase-14.md` 新验收项、coverage matrix。
- **单独一项，不捆绑**：若要让 `docs/plan/` 的修改只影响相关章节的模块（如 07 只影响编辑/输入项），需要另改
  plan 11 中"`docs/plan/`、`docs/acceptance-cases/` 对全部功能模块保守传播"一句。这是唯一能让 v0.1.3 这类
  "改了一章 plan"的版本不全量失效的改动，但它放宽合同传播，需单独评估。

### 成本

| 项 | 成本 |
| --- | --- |
| 人工记录类型、人工策略、readiness 分支 | 中 |
| FAIL 保留与条件写入（存储层新增） | 小到中 |
| 场景表与逐场景记录（引导与逐项两个记录器） | 中 |
| 主机事实采集（`ver`、`reg query`、字体目录清单） | 小 |
| plan 11、术语、投影文档 | 中 |
| 根因 4 第 1 步：分类诚实化 + 守卫 | 小 |
| 根因 4 第 2 步：拆分 `input.rs` | 中；下一个候选全量重跑；收益只在职责边界 |
| 根因 4 第 3 步：逐项缩窄 | 证据成本高，按上文分析收益可能接近 0 |

### 选项与建议

- **选项 1（完整）**：上述修正后的根因 3 + 根因 4 全部三步。
- **选项 2（最小，建议）**：根因 3 按修正后的设计实施（产品域保守取全部产品域，IME 项与 M34 绑定 ZIP）；根因 4
  只做第 1 步。`input.rs` 拆分以后按职责边界单独做，不以复用为理由；逐项缩窄等有低成本的证据方法再议。

建议选项 2。理由：正确性优先；有时效性的是**记录格式**——下一次完整人工基线（无论落在 v0.1.3 重建还是
v0.1.4）只有按新格式记录，之后才可能复用；而缩窄产品域的收益小、证明成本高。

分批（选项 2）：根因 4 第 1 步 → 根因 3（plan/术语/投影文档一批，工具实现一批）。每批经 gpt-6-astra 审查后
提交并推送。

## 修订（第 2 轮审查后，2026-10-10）

第 2 轮：第一轮 7 项中 5 项判为已修、2 项部分修复；新增 2 个 BLOCKER 与若干 SHOULD_FIX，核实后全部成立：
字体按内部 family 名解析（`fonts.rs`；fontdb 0.23.0 `load_system_fonts` 读取 `%SYSTEMROOT%\Fonts` 与用户
`AppData\Local`、`AppData\Roaming` 下的 `Microsoft\Windows\Fonts`，cosmic-text 0.19.0 按系统 locale 选择回退），
文件名+大小不是内容身份；产品主题取 winit 的有效主题（`lifecycle.rs:86`），不是两个注册表偏好值；plan 11:319、
:433、`docs/acceptance-cases/AGENTS.md:16` 与 `phase-12.md` 前言仍写着"当前提交/exact candidate 收据"。
本节修正第 1 轮修订；冲突处以本节为准。

### 环境模型：被测环境、目标环境、readiness 所在机器三者分开

第 1 轮修订把"readiness 时与当前主机比较"作为环境判定，这是错的：开发机、Clean VM、前一个 Windows 版本的
机器各自产生合法观察，不可能同时等于执行 readiness 的那台机器。修正为：

1. **被测环境身份**：记录器在被测机器上采集，随观察写入，并在提交时重新采集比对（不一致即拒绝登记）。字段：
   Windows 功能版本与 edition（`reg query` 读 `DisplayVersion`、`EditionID`）、四段构建号（`cmd /d /c ver`，
   恰好四个数字段）、系统 locale、高对比度状态、**字体内容摘要**（fontdb 读取的三个目录下全部字体文件，按排序后
   的相对路径流式计算 SHA-256，不整份读入内存；本机约 618 MB + 97 MB，实现时实测耗时）。任一字段采集失败、
   超时或格式不符时，这次观察不能登记为有效 PASS。
2. **目标环境档案**：每次 Source Freeze 后，观察者在每台目标机器上运行一次档案采集，生成绑定该 Source Freeze
   的档案收据（字段同上）。这份收据就是"本次发布要求覆盖的环境"的当前陈述；需要哪些档案由合同决定
   （见下文待决事项），不能从已有 PASS 反推。
3. **readiness**：对每个"项 × 必需档案"，有效当且仅当存在指纹兼容、场景完整、被测环境身份等于该档案本次
   采集身份、状态为 PASS 的最新观察。readiness 在哪台机器运行不影响判定。

因此 Windows 打补丁后，下次档案采集的身份不同，该档案下的项需要重测；没打补丁、没装卸字体、没改 locale
时可以复用。

### 不复用的项（绑定当前候选 ZIP）

- 断言 IME 行为的 7 项：M01、M02、M05、M18、M22、M36、M41（第 2 轮确认清单与 `phase-12.md` 一致）。
- M34：ZIP 只证明制品身份，SmartScreen 信誉随下载历史变化；同字节重新晋升也重测。
- M42：RDP 客户端版本与连接设置不在采集范围内。

其余 15 项可按上面的环境模型复用。

### 主题

M23 的场景是 Light、Dark、System 与 System 下的运行时切换（两个方向）及实际结果；最终停在哪个主题不影响覆盖。
其他视觉项把主题作为前置条件（Light，非高对比度）记录在场景里。注册表的主题偏好值只作观察元数据。高对比度
开启时该次观察不能登记为有效 PASS（合同没有覆盖高对比度）。

### 逐项场景表

"必需场景"全部完成且 PASS 才能登记 PASS；任一场景 FAIL 立即登记该项 FAIL。场景参数由观察者声明，来源标为
"声明"。

| 项 | 必需场景 | 前置条件 | 合同来源 | 复用 |
| --- | --- | --- | --- | --- |
| M01 | 微软拼音 × 100/150/200% | Light | plan 07 Verification；功能由 G4-06 自动化 | 否 |
| M02 | WeChat/WeType × 100/150/200%；环境缺失记 NOT TESTED | Light | 同上 | 否 |
| M05 | Alt+Tab 离开后分别经点击、托盘、传感区恢复并输入 | — | plan 09 Tool Window/焦点/IME | 否 |
| M11 | 真实 mixed-DPI 双屏，左 dock：失焦收起、传感区展开、DIP/物理换算 | 双屏 mixed DPI | plan 09 dock、plan 11 :608 | 是 |
| M12 | 同上，右 dock | 同上 | 同上 | 是 |
| M18 | 220×120 Source 输入与滚动，caret/selection/IME 不被遮挡 | — | plan 09 220×120 | 否 |
| M19 | 220×120 Preview 滚动、选择、链接 | — | 同上 | 是 |
| M20 | 220×120 Split 输入与两栏滚动 | — | 同上 | 是 |
| M21 | 50/100/300% × Source/Preview/Split；滚轮缩放与 reset | — | phase-12 M21 | 是 |
| M23 | Light、Dark、System、System 下双向运行时切换 | 非高对比度 | plan 09 主题 | 是 |
| M24 | 代表性 Markdown fixture 的 Preview 视觉与选择/链接 | Light | phase-12 M24 | 是 |
| M25 | 正确与错误公式 fixture | Light | phase-12 M25 | 是 |
| M26 | PNG/JPEG/WebP/GIF、超限图片、视口下方图片的滚动显示 | Light | phase-12 M26 | 是 |
| M34 | Clean Windows 11 VM 解压运行、无额外 runtime、信誉提示与 README 一致 | Clean VM 基线（声明） | plan 11 | 否 |
| M35 | 双屏同 DPI：拖动、dock、托盘恢复 | 双屏同 DPI | plan 09 双显示器 | 是 |
| M36 | 双屏 mixed DPI：拖动、dock、IME、Preview | 双屏 mixed DPI | 同上 | 否 |
| M37 | 运行中断开当前显示器后恢复窗口 | 双屏 | plan 09 拔线 | 是 |
| M38 | 125%：输入、Preview、dock | 单屏 125% | plan 11 :91 DPI | 是 |
| M39 | 150%：同上 | 单屏 150% | 同上 | 是 |
| M40 | 200%：同上 | 单屏 200% | 同上 | 是 |
| M41 | 睡眠恢复后输入、保存、dock | — | plan 09 sleep/resume | 否 |
| M42 | RDP 重连后输入、保存、dock；无环境记 NOT TESTED | RDP | plan 09 RDP | 否 |
| M43 | 副屏在左（x<0）与在上（y<0）各一次：恢复、dock、比例保存 | 物理负坐标布局 | plan 09 双显示器左侧/上方 | 是 |

场景表放在工具内（带 `plan_ref`），属于 harness 输入；改动场景表即使相关项失效。

### 生命周期（补全）

- 状态分四种：`Absent`（无记录）、`Pass`、`Fail`、`Invalid`（记录存在但 evidence 不可读、摘要不符或校验失败）。
- 当前指纹 × 当前档案下的 `Fail` 与 `Invalid` 都阻断，且优先于 waiver 与 Tier C；`Invalid` 不回退到更早的 PASS，
  不当作 NOT_TESTED。恢复方式只有一次正式重测。
- 其他指纹或其他档案下的历史 FAIL：显示为历史，不阻断当前判定，也不称为已修复；P0/P1 问题仍走独立的发布阻断。
- 只有登记 PASS 要求场景完整；任一场景观察到 FAIL 立即登记该项 FAIL，之后中断不会丢失它。
- 开始观察与提交时都核对 Source Freeze、候选、项指纹与被测环境身份；任一变化即拒绝登记。条件写入冲突时必须
  重新读取当前记录并重新观察，不自动刷新版本后重交旧答案。
- 正式人工重测与普通诊断分开：前者写入账本，后者不写（plan 11 :433 需相应修改）。

### plan 修改范围（追加）

除第 1 轮修订所列，还需修改：plan 11 :319（readiness 接受的证据类别加入人工观察）、:433（正式人工重测与诊断的
区别）、Owned Objects/Outputs 与人工操作说明；`docs/acceptance-cases/AGENTS.md:16`；`phase-12.md` 前言；
新增目标环境档案收据。自动化的 Last Successful Module Receipt 术语与失败策略保持原义。新人工证据校验器与
场景表属于全局或 harness 输入，不能归入"只做展示"的排除项。汇总不能单独授权 READY，必须能从记录引用重新验证。

### 待 USER 决定

1. **plan 11:91 的"当前与前一个受支持 Windows 11 版本"**。工具目前只记一个 Windows 构建号，从未执行这一要求。
   - (a) 按合同建模：必需档案 = 当前版本 + 前一个版本（按功能版本与 edition 区分，不是两个补丁号）；缺档案时
     NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver。需要第二台机器或 VM。
   - (b) 修改合同：前一个版本改为可选。
   建议 (a)：不放宽合同，缺口以 waiver 显式可见，与现有发布特例的做法一致。
2. **选项 2 是否采纳**（维持第 1 轮修订的建议）。

## Resolution（2026-10-10）

USER 选择：plan 11:91 按 **(a)** 建模（当前与前一个受支持 Windows 11 版本均为必需目标环境档案，缺档案时
NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver）；采纳**选项 2**（根因 3 按修正后
的设计实施，产品域保守取全部产品域；根因 4 只做第 1 步）。第 3 轮设计审查的结论落入实施前的最终设计。

## 第 3 轮设计审查（2026-10-10，USER 决定之后收到）

gpt-6-astra（max）第 3 轮：第 2 轮意见多数已修；新增 3 个 BLOCKER，均针对**跨候选复用**本身：

1. "本次采集"与"不复用"缺少可校验的轮次身份：Source Freeze 与 Candidate 只含内容字段，同源再次 Freeze 或同字节
   再次晋升身份相同，旧档案可以冒充新一轮采集。需要本轮资格化/晋升标识，由现场采集器读取，且不能混入可复用项
   的内容指纹。
2. 条件写入冲突仍可吞掉已观察到的 FAIL（A 发布 PASS 后 B 的 FAIL 提交被拒，B 中断，账本仍是 PASS）。冲突中的
   正式失败必须留下阻断记录，或对同一观察键单会话排他。
3. 环境采集没有绑定实际被测进程：先启动产品再改字体或 locale，开始与提交两次采集都是新环境，产品却仍用启动时
   的字体数据库。正式会话应在采集后启动并绑定被测实例。

SHOULD_FIX：locale 应为 `GetUserPreferredUILanguages`（cosmic-text 经 sys-locale 读取），不是系统 locale；
fontdb 按未排序的目录枚举加载，同 family/style/weight 时先出现者胜，排序后的内容集合抹去了这一顺序；显示适配器、
驱动与本地/虚拟会话身份未采集（产品 CPU 光栅化，但经 GDI `BitBlt` 与 DWM 合成，并使用 layered-window alpha）；
场景表漏了 M22（本提案新增的错误；补回后仍是 9 项不复用、15 项可复用）；多机器观察如何进入同一账本未定义
（存储只在同一 clone 内共享）；(a) 的版本选择（架构、servicing channel、edition、冻结日期）、项与档案的适用关系、
环境级 waiver 的政策含义未定。另指出 plan 09 手工矩阵中 Esc、手动收起与透明度 70/96/100 在 P12 没有归属，
属于**既有**缺口。NEEDS_VERIFICATION：字体摘要只读实测约 688 MiB、WSL 流式 SHA-256 约 7.5–8.8 s；若每项开始
与提交各扫一次，每环境读取量约 33 GiB，需要合并采集。

审查同时建议把根因 3 分两步：**先建立新格式与完整观察记录（仍绑定当前候选，不跨候选复用），再在上述身份问题
闭合后开启复用**。这是已批准范围内的先后顺序，不缩小范围；采纳为实施顺序：

1. 根因 4 第 1 步（本批，见下节）。
2. 根因 3-A：逐项逐场景的观察记录、目标环境档案、两个 Windows 版本的覆盖、FAIL 优先与四种状态，**仍绑定当前
   候选**；补回 M22，并处理 Esc/手动收起/透明度档位的归属。
3. 根因 3-B：轮次身份、进程绑定、冲突下的失败持久化、locale 与字体加载顺序、显示栈身份、多机器导入，闭合后
   开启跨候选复用。

## 根因 4 第 1 步实施记录（2026-10-10）

- `fingerprint.rs` 的产品域改为三张显式表：`EVERY_SESSION`（每次会话的启动路径或事件循环每轮执行的代码，归全部
  产品域）、`NOT_YET_NARROWED`（此前落入兜底、尚未追踪闭包的文件，保持全部产品域）、`FEATURE_SOURCES`（只经某个
  功能到达的代码与其域）。没有显式规则的产品文件运行时仍归全部产品域。
- 新守卫 `every_product_source_has_an_explicit_rule`：真实 checkout 中落入兜底的产品文件与不再命中任何 tracked 文件
  的规则都使测试失败；两种变异（删一条规则、加一条不存在的规则）均实测失败。
- **只放宽不缩窄**：172 个产品相关文件逐一比较前后掩码，没有任何文件的掩码失去位；71 个文件掩码变宽。对四个窄模块
  新增的失效：
  - 16 个窗口文件（`app/{controls,toolbar_paint,window_geometry_runtime,window_interaction,window_runtime}.rs`、
    `flow/window/*`、`platform/windows/{monitor,native_message,tool_window,tray,window_opacity,window_topmost}.rs`）
    新增使 Performance 与 G3 失效——它们在启动路径上（例如 `window_runtime.rs` 记录 `guards_ready` 启动诊断，
    `lifecycle.rs` 启动时创建托盘、应用透明度与 Tool Window 样式），原先只归 `SHELL`；
  - 16 个启动、note 加载与持久化文件（`startup/*`、`persistence/*`、`flow/persistence.rs`、`*_runtime.rs` 中的持久化
    胶水、`platform/windows/{file_watch,program_dir,single_instance}.rs`）新增使 G5 失效——G5 渲染用例写入 note 后由
    `startup/bootstrap.rs` 经 `persistence` 加载；
  - `app/export_runtime.rs` 新增使 Performance 与 G4 失效（它是 `StickyApp` 胶水，`export_in_flight` 被窗口守卫读取）。
  其余变宽（如 `app/input.rs` 从 `EDITOR` 到全部产品域、剪贴板加 `ASSETS|IMAGES`、`atomic_file.rs` 加 `EXPORT`）不改变
  现有模块判定，但对日后的更细消费者是诚实的。
- 模块掩码核对：导出是保留源文本的 Markdown，只经 `render/preview` 的图片引用改写（该目录掩码含 `EXPORT`），不经数学
  渲染；G3 掩码不含 `MATH` 成立，未修改任何模块掩码。
- plan 11 `#module-success-ledger` 增加分类原则一段；`phase-14.md` 新增 P14-A72；coverage matrix 与治理范围同步到 A72。
- 验证：Windows `cargo test -p stickymd-smoke --locked` 396 + 25 通过（17 ignored），clippy `-D warnings`、fmt、
  phase-00 治理通过；WSL Debian 回放 Linux CI：clippy 通过，326 + 19 通过。

### 代码审查第 1 轮（2026-10-10）

gpt-6-astra（max）复现了"172 个文件、71 个变宽、0 个丢位"与 `domains()` 不变，但给出 2 个 BLOCKER、2 个 SHOULD_FIX，
核实后全部成立并已修正：

- BLOCKER：`flow/{save,recovery,reconciliation}.rs` 只归 `PERSISTENCE|STARTUP`，与 G5 无交集；但 `StickyApp::new`
  创建 `RecoveryCoordinator`，事件循环每轮调用 `tick_autosave`，G5 粘贴用例等待 note 落盘后又经 reconciliation。
- BLOCKER：`atomic_file.rs` 与 G5 无交集；`config/storage.rs` 的 `save_config` 经 `atomic_publish` 写配置，G5 切换视图
  后等待配置落盘。
- SHOULD_FIX：只改产品源码的定向 CI 跳过 smoke 包测试，守卫不一定运行。改为由 `phase 00` 治理调用
  `verify_product_classification`（CI plan 作业每次运行），单元测试调用同一函数；实测在未分类产品文件出现时
  `phase 00` 失败。
- SHOULD_FIX：`flow/editor.rs`、`interaction/search.rs` 在 `app.rs` 启动时初始化，`flow/preview.rs` 启动时 `show`、
  每轮 `tick`，`assets/{storage,safe_boundary}.rs` 在 `main.rs` 启动时执行，caret overlay 在默认 Source 首帧后使用；
  这些已移入 `EVERY_SESSION`。`app/` 下只处理导出等功能的文件仍归全部产品域，注释改为如实说明理由：它们共享
  `StickyApp` 可变状态，每轮代码读取（如 `export_in_flight`、`asset_paste_pending` 进入窗口守卫）。

修正后对四个窄模块新增的失效：Performance 17、G3 16、G4 1、G5 20 个文件（172 个文件中 81 个掩码变宽，0 个丢位）。
新增回归 `every_session_persistence_invalidates_every_module`：note 加载、恢复检查、autosave、reconciliation、配置存储、
原子发布与资产存储文件都必须使每个模块失效。plan 11 分类段落与 P14-A72 同步补入编辑后的持久化与治理检查。
验证：Windows 397 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、327 + 19 通过、phase-00 通过。

### 代码审查第 2 轮（2026-10-10）

第 1 轮修复全部属实，复算仍为 81 个变宽、0 个丢位。1 个 BLOCKER（自 HEAD 遗留）：正式 Performance 计划按名称过滤运行
Release 单元基准，其中 Phase 7 的 `phase7_export_release_baseline` 位于 `export/mod.rs` 并调用 `export_snapshot`，但
Performance 掩码不含 `EXPORT`，只改导出代码时旧 Performance 记录仍可复用。修正：Performance 掩码加入 `EXPORT`
（导出目录因此新增使 Performance 失效，合计 18 个文件）；新增回归 `performance_baselines_outside_its_features_invalidate_it`。
逐一核对含 Performance 基准的产品文件，修正后掩码均与 Performance 相交。其余结论：剩余 `FEATURE_SOURCES` 与各模块
实际执行路径一致（G3 五个用例都不切换 Preview/Split，导出不经数学排版；Performance 的 typical seed 含行内公式但以
Source 视图启动，不做数学排版）；治理调用 `git ls-files` 与 CI plan、release 工作流兼容，没有 Git 元数据的源码副本会
失败，现有工作流不使用这种环境。

未做：按 Performance 任务的名称过滤自动推导"含基准的文件必须与掩码相交"的通用守卫；当前以回归列出已知文件。

验证：Windows 398 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、328 + 19 通过、phase-00 通过。

## 根因 3-A 设计稿（2026-10-14，待审查）

范围：人工验收的**记录模型**——逐项逐场景、目标环境档案、两个 Windows 版本的覆盖、FAIL 优先、多机器导入。
观察仍绑定当前候选（source/EXE/ZIP/version），**不做跨候选复用**；字体、locale、显示栈身份、轮次身份、进程绑定与
条件写入属于复用（3-B），本批不实现。理由：3-A 中环境身份只用于判定"这条观察覆盖了哪个目标 Windows 版本"，
功能版本、edition 与架构不会在一次会话内变化；而字体/locale 只影响"旧观察能否代表新环境"，那是 3-B 的问题。

### 已核实的现状

- phase-12 共 24 个 Manual 项（14 Tier A、7 Tier B、3 Tier C）；P12-M44 是 exact 自动化项，不在人工集合中。
- Esc 收起与手动收起由 Phase 8 reducer 自动化持有（P08-A05、P08-D058..D060）；透明度 40/70/96/100 视觉是
  P08-M06，P12 只有 M22 的 40%。plan 09 Verification 把它们笼统列为"手工矩阵"，与现行分工不一致。
- 全工作区（含 smoke 工具）以 `+crt-static` 构建，工具二进制可在未装 VC runtime 的机器上运行。
- 现有收据单文件 `dist/evidence/manual-acceptance.json`（schema 1），每项一个状态，Windows 构建只记一个字段。

### 设计

1. **用例与场景注册表**（工具内，`plan_ref` 指向 plan 11）：24 项与 phase-12 的 Manual 集合精确相等（治理检查），
   每项列出必需场景（第 2 轮修订的场景表，补回 **M22：透明度 40/70/96/100 × 输入、预览、IME**，吸收 P08-M06 进入
   发布矩阵）。plan 09 Verification 改为区分：Esc/手动收起归 Phase 8 自动化，透明度四档归 M22。
2. **目标环境档案**：两个角色 `current`、`previous`。Source Freeze 之后由 USER 以交互命令声明一份目标收据（绑定
   Source Freeze 身份），每个角色写明 Windows 11 功能版本（如 `25H2`）、edition 族（Home/Pro）与架构（x64）。plan 写明
   选择规则：冻结日期时、面向通用设备的 General Availability Channel 中仍在服务期的最新与次新功能版本，排除只面向
   特定新设备的分支；工具不联网，不自行推断。
3. **观察记录**（收据 schema 3，仍是同一文件、仍绑定当前候选）：只追加的观察日志，每条为
   `{seq, case, scenario, role, status ∈ {MANUAL_PASS, MANUAL_FAIL}, note, observed, declared, session}`。
   `observed` 由记录器在被测机器上采集：`DisplayVersion`、`EditionID`（`reg query`）、四段构建号（`cmd /d /c ver`，
   恰好四个数字段）、架构；任一采集失败则拒绝登记。`declared` 为场景参数（DPI、拓扑、主题等），标注来源为声明。
   观察的 `role` 由 `observed` 与目标收据匹配得出，不由观察者选择；匹配不到任何角色则拒绝登记。
4. **状态推导**（每个 `(项, 角色)`）：`Absent`（无观察）→ NOT_TESTED；某场景最新观察为 FAIL → `Fail`；全部必需场景
   最新观察为 PASS → `Pass`；日志损坏或身份不符 → `Invalid`。FAIL 在该场景观察到时立即追加，之后中断不会丢失；
   只有该场景之后的 PASS 才覆盖它。NOT_TESTED、跳过、中止从不写入。
5. **并发**：同一工作区同一时刻只允许一个记录会话（`dist/evidence/` 下的 OS 排他锁，进程退出即释放）；追加在锁内
   完成并原子替换收据。3-A 只有一个写入入口（本地记录或导入），因此不需要条件写入。
6. **多机器**：其他机器（前一 Windows 版本、Clean VM）用工具二进制的**观察包**模式记录：只需候选 ZIP，不需要仓库；
   候选身份由本地计算的 ZIP/EXE 摘要给出，包内含目标收据摘要、采集环境与观察。主工作区 `import` 在锁内严格校验
   （schema、候选身份与当前候选一致、目标收据摘要一致、项/场景已知、无重复 seq），按包内顺序追加。M34 在 Clean VM
   中先完成 StickyMD 首次运行的观察，**之后**才复制并运行工具记录（工具是静态链接的单文件，不安装 runtime）。
7. **readiness**：每个 `(项, 角色)`：`Fail`/`Invalid` 阻断，且优先于 waiver 与 Tier C；`Absent` 按 tier 与 waiver 处理；
   全部 `Pass` 为 `MANUAL PASS`。waiver：保留逐项 key 与 Tier B 组 key；新增档案级 key
   `WAIVER-PROFILE-PREVIOUS-v{version}`（绑定 Source Freeze），只把 `(项, previous)` 的 `Absent` 处置为 `USER WAIVED`，
   不覆盖 FAIL/Invalid。汇总逐项逐角色列出状态、场景覆盖与 waiver 引用。
8. **引导记录器**：一步覆盖多项时逐项询问；一项多场景时逐场景询问。
9. **迁移**：schema 1 收据缺少场景与角色，不导入；readiness 报告其已被取代。

### 待 USER 决定（GPT 审查后带建议提交）

- **项 × 角色的适用关系**：(i) 全部 24 项在两个角色上都做（严格读法，人工时间约翻倍）；(ii) 全部项做 `current`，
  `previous` 只做依赖 OS 集成的子集（需在 plan 中逐项列出）。注意产品字体来自系统字体，纯渲染项也随 Windows 版本
  变化。
- **档案级 waiver 的结论**：缺 `previous` 时以 `USER WAIVED` 计入 READY（与现有逐项 waiver 一致），还是保持技术
  `NOT_READY`、走发布特例。

### plan 修改（3-A 部分）

plan 11：:91 角色与选择规则；:237 `MANUAL PASS` 定义改为逐项逐角色、场景完整；:258 waiver（Tier B 组 key 与档案 key）；
:336–345 记录器（场景、逐项、会话锁、观察包导入、Windows 四段）。`01_terminology.md`：Target Environment Profile、
Manual Observation。plan 09 Verification 分工。`phase-12.md`：M22 场景。`phase-14.md`：新验收项。复用相关的 :243、
:316 与 `#module-success-ledger` 人工策略留给 3-B。

### 3-A 设计审查第 1 轮与修订（2026-10-14）

gpt-6-astra（max）：5 个 BLOCKER、3 个 SHOULD_FIX、1 个 NEEDS_VERIFICATION，核实后全部成立。修订如下（与上节冲突处以
本节为准）。

1. **迟到的观察包不能清除较新的 FAIL。** 每条观察有稳定 ID（`session_id + local_seq`，重复导入幂等）与 `base`：会话开始
   （或主工作区导出观察任务）时日志的登记长度。某场景的 PASS 只有在 `base` 已包含该场景的最新 FAIL 时才覆盖它——即观察者
   看到过这个失败并做了正式重测；否则 FAIL 保留，状态为 `Fail`。不需要分布式锁或向量时钟。
2. **上下文身份。** 日志头绑定 candidate receipt 摘要（含 workflow run/attempt/artifact id；新一次构建即新身份）、目标收据
   摘要与场景注册表摘要。目标收据含随机上下文 ID，日志存在后不可改；要改目标必须归档当前日志、建立新上下文，旧 waiver
   不继承。同一 artifact 原样重新晋升得到同一 candidate，不视为新一轮（3-A 不建模信誉等随时间变化的外部状态；第 2 轮修订
   中"M34 同字节重新晋升也重测"属于 3-B 的复用规则）。
3. **目标匹配谓词。** 目标收据逐角色记录：选择日期与微软依据（release health 表的版本与日期）、功能版本、对应构建分支
   （如 25H2 ↔ 26200）、允许的 `EditionID` 白名单（具体 SKU，不用 Home/Pro 这类政策标签）、原生架构。采集：
   `DisplayVersion`、`CurrentBuild`、`UBR`、`EditionID`、原生 `PROCESSOR_ARCHITECTURE`（`reg query`）、Insider 登记状态
   （`WindowsSelfHost\Applicability`）。角色当且仅当功能版本、构建分支、SKU、架构全部相符且未登记 Insider 时匹配，且
   必须唯一；`DisplayVersion` 缺失或匹配不唯一即拒绝登记。导入与 readiness 重新计算角色。审查查得按微软当前发布表，
   2026-10-14 的两个目标应为 26H2 与 25H2——这是 USER 声明时的事实，工具不联网推断。
4. **完整的状态推导**（每个 `(项, 角色)`）：证据无效 → `Invalid`；任一必需场景的有效最新观察为 FAIL → `Fail`；全部必需
   场景 PASS → `Pass`；其余 → `NOT_TESTED`，附已覆盖/缺失场景（零覆盖与部分覆盖只是元数据）。waiver 处置的是未完成的
   `(项, 角色)`，不删除已有 PASS。
5. **覆盖映射，而不只是 24 项。** 每条 plan 07/09/11 人工断言与每个 Phase 8 人工行（P08-M01..M15）必须映射到 P12 人工项
   的场景、或已批准的 exact 自动化入口，或经批准注明退役。逐行核对的结果：
   - 已覆盖：P08-M01/M02（M11/M12 + M38–M40）、M04/M13（P14-A30、G4-06 + M01/M02）、M05 的托盘菜单与生命周期
     （P12-M06..M09，G4-01）、M07（M23）、M08（P12-M44，G4-05）、M11（M36、M43）、M14（M24、M25、M23、M38–M40）、
     M15（M26、M28–M30）。
   - **缺口**（早于本提案）：顶部 dock 视觉（P08-M03 的视觉部分；时序由 M10/G4-02 自动化）；Alt+F4 隐藏到托盘
     （P08-D031，无自动化）；两份复制目录互相独立（P08-M09）；主显示器切换与上下排列（P08-M10）；Windows 11 外壳视觉
     契约——圆角、阴影、置顶按钮、控件淡化、自定义缩放、辅助功能（P08-M12）；DPI/主题下滚动条可抓取且不遮字
     （plan 09）；真实 Esc 与按钮收起（reducer 由 P08-A05 自动化，但真实键盘入口在 composition 时先取消 preedit 并返回，
     不进入窗口 intent，二者不等价）。
   - 处理：Esc/按钮收起（含 composition 中按 Esc 只取消 preedit 的预期）并入 M11/M12；顶部 dock 视觉并入 M38–M40 的
     dock 场景；滚动条并入 M23 与 M38–M40；主显示器切换与上下排列并入 M35；Alt+F4 并入 M05；新增两项 Manual：
     **外壳视觉契约**与**两份便携副本互相独立**（新 ID 接在 P12-M44 之后）。M22 吸收 P08-M06 的全部断言（背景、文字、公式、
     图片、控件、阴影与滑块/数字输入提交）。治理检查"每个 P08 人工行都有映射"。
6. **观察包与被测实例。** 主工作区导出观察任务（候选 receipt 摘要、目标摘要、场景注册表摘要、`base`）；独立模式校验 ZIP
   及其中 EXE 与任务一致，导入复用现有 candidate 校验路径。记录器把已校验的 ZIP 解压到会话目录、按场景放入 fixture，
   自行启动该 EXE，并在观察中记录 EXE 摘要与进程 ID；观察者只观察这个实例。M34 例外：Clean VM 中先完成首次运行观察，
   之后才运行工具，观察标为人工声明并记录 VM 基线与包的取得方式。威胁模型如实写明：信任 USER，不防伪造观察。
7. **锁与生命周期。** 独立且不被替换的 sidecar 锁 `dist/evidence/manual.lock`（每个 worktree 各一份；`dist/evidence/` 本就
   按 worktree 隔离），覆盖"读旧状态到原子提交"；记录、导入、目标声明取排他锁，readiness 取共享锁；等待有上限。FAIL 在
   可选备注之前落盘；写失败非零退出；观察包整体校验后一次提交。日志损坏为 `Invalid` 并阻断；重建日志需要 USER 决定 key
   `RESET-MANUAL-LOG-<digest>`，汇总中可见。上限：观察包 1 MiB、单个日志 10 000 条、备注 2 KiB。
8. **plan 修改补全。** 本批就在 plan 11:316 处说明人工观察是独立的证据类别（含 FAIL，绑定当前候选），不是 Last Successful
   Module Receipt；同步 Owned Objects/Outputs、readiness 接受规则、上下文失效、失败恢复、引导记录、waiver 范围；plan 09
   Verification 的分工；Phase 8→12 映射；`phase-12.md` 前言、全部场景与两个新项；`phase-14.md`；coverage matrix。:243 的
   旧 commit 禁令在 3-A 仍成立，保留。
9. **待实测**：实际分发的工具 EXE 的导入表，以及在没有仓库、Rust、VS、额外 VC runtime 的 VM 中运行独立模式。

审查对两项 USER 决定的意见：(i) 两个角色都覆盖完整注册表（仍按 tier/waiver 规则处置，不是 48 项无条件 PASS）；档案级
waiver 计入 READY 并在汇总中逐项列出 `USER WAIVED`，同时写明这是"发布资格满足批准政策"而非"技术验收全部完成"；目标
收据缺失、损坏或含糊属于 authority 无效，不能用同一 waiver 绕过。

### 3-A 设计审查第 2 轮与修订（2026-10-14）

第 2 轮：只追加日志内的 `base` 规则、部分完成状态、候选身份（同一原始 artifact 原样重新晋升视为同一 candidate）已成立；
新增 4 个 BLOCKER、2 个 SHOULD_FIX，核实后全部成立。不涉及政策的部分修订如下：

1. **日志重建即新上下文。** 任何清空、重建或丢失日志后的恢复都生成新的随机上下文 ID；旧任务、旧观察包与旧 waiver 都不
   继承。同 ID 同内容的重复导入是 no-op（不重新分配序号），同 ID 不同内容拒绝。记录器在 FAIL 落盘后提示"重新开始重测
   任务"，提交旧答案时不自动刷新 `base`。
2. **观察任务携带内容而非只有摘要**：完整的候选 receipt、目标收据与场景注册表片段随任务导出，无仓库的记录器据此校验
   ZIP/EXE。
3. **按场景定义被测实例**（复用 exact harness 现有的候选校验、解压与逐用例副本设施）：同目录单实例场景用同一 canonical
   目录或其别名第二次启动；两份副本项登记两个目录两个进程；M05 识别被测实例的托盘项；M37/M41/M42 保留原进程与 note，
   不得靠重启或重新 seed 恢复，预期断连期间的环境探测不判产品 FAIL；M34 保存首次运行的人工声明、VM 基线与取得方式，工具
   后来启动的实例不替代首次运行事实。会话目录正常与异常退出都有清理和有界保留；日志锁不代替交互桌面独占，记录器开始前
   检查没有其他 StickyMD 实例。
4. **注册表驱动 tier 与 waiver 范围**：ID、tier、必需场景、waiver 范围都由同一注册表声明，记录器、校验与 `decisions.rs`
   的 key 校验都读取它，不再依赖 `M01..M44` 的编号范围。
5. **plan 修改补充**：`01_terminology.md` 的 Promoted Candidate lifetime 为 3-A 的同 artifact 人工上下文写出窄范围规则，
   同时保留每次 Promote 重验 exact-byte evidence；plan 11:559"任何人工观察前先检查环境"为 M34 写明例外（事后记录的
   是人工声明，不冒充事前环境证据）。
6. **"非 Insider"改为可执行的判定**：不探测 Insider 登记状态（没有一手契约保证该注册表键能识别所有通道，尤其 Release
   Preview）。目标收据为每个角色列出 USER 依据微软发布历史声明的**获准完整构建号**（`CurrentBuild.UBR`）及来源；观察的
   完整构建号必须在列表内。构建号相同即同一份系统位，与机器登记在哪个通道无关。原生架构取 `IsWow64Process2` 定义的
   原生机器类型（不用进程环境变量）。注册表读取写明路径、视图与类型；任一项读取失败即拒绝登记。代价：资格化期间机器
   打补丁后构建号不在列表内，需要新上下文。

### 待 USER 决定（第 2 轮后）

1. 项 × 角色：(i) 两个角色覆盖完整注册表 / (ii) `previous` 只做子集。审查与我都建议 (i)。
2. 缺 `previous` 档案：READY 并逐项列出 `USER WAIVED`（写明"符合批准的发布政策"而非"技术验收全部完成"）/ 保持 NOT_READY
   走发布特例。审查与我都建议前者；目标收据缺失、损坏或含糊不能用 waiver 绕过。
3. **Esc 语义（产品合同）**：plan 09 写"Esc/手动按钮无条件立即收起"，实现中 IME composition（取消 preedit）与 Search 先消费
   Esc。建议把输入优先级写进 plan 09：composition、Search、弹出控件先消费，未被消费时才进入窗口收起。
4. **覆盖缺口的处理方式**：Phase 8 人工义务映射到 P12 需要逐断言粒度（左右 dock 的 100% 与 `>16 DIP` 脱离、Right/Top dock
   的 IME 路径、保存失败时取消退出、Topmost 与托盘同步、最小化/dock 状态下第二次启动与前台激活、Alt+F4、两份副本的
   托盘区分、外壳视觉与边缘/角落调整大小等），部分可扩充 G4 自动化，其余为人工场景，可能新增两项 Tier A（外壳视觉、两份
   副本），注册表变为 26 项 × 2 角色。这些缺口早于本提案。建议**单独作为 3-A 之后的一批**（覆盖补全），3-A 先在现有 24 项
   上建立记录模型，缺口在报告与汇总中如实列出。

## Resolution（2026-10-14，3-A 范围）

USER 选择：(1) `previous` 角色覆盖完整注册表；(2) 缺 `previous` 档案时以 USER waiver 计入 READY，汇总逐项列出 `USER WAIVED`
并写明"符合批准的发布政策"，目标收据缺失、损坏或含糊不能用 waiver 绕过；(3) Esc 输入优先级写入 plan 09：IME composition、
Search、弹出控件先消费，未被消费时才收起窗口；(4) Phase 8 人工义务的覆盖补全作为 3-A 之后单独一批，3-A 在现有 24 项上
建立记录模型，缺口在报告与 readiness 汇总中如实列出。

### 3-A 文档批次起草中发现的问题（2026-10-14）

核对 Esc 的真实消费顺序（`input.rs`、`window_interaction.rs`、`preview_input.rs`）：透明度数字输入 → 恢复类模态守卫 →
Search → **Preview 焦点（Esc 只清除 Preview 选区，从不收起窗口）** → IME composition → 窗口收起。USER 批准的优先级只列了
composition、Search 与弹出控件；Preview 焦点下 Esc 永不收起窗口这一点未经批准，plan 09 草稿暂未写入，待 USER 决定：
保持现状并写入合同，或改为"有 Preview 选区时清除选区，否则收起"（需要改产品代码）。

### 3-A 设计审查第 3 轮（2026-10-14）与 Resolution

第 3 轮（两次因服务端满载失败后的第三次运行）：模型主体收敛，唯一 BLOCKER 是 Esc 行为表未闭合；SHOULD_FIX：构建号相同
不等于系统位或环境等价、注册表类型与逐场景采集时点、观察任务需携带待解除的 FAIL、所有人工 waiver 绑定上下文与
`(项, 角色)` 集合；NEEDS_VERIFICATION：独立记录器需拆开仓库校验入口并实测。另建议场景归约与登记顺序无关、readiness
始终输出已知缺口、读取前先查字节上限。均已写入 plan 11 草稿。

USER 决定（2026-10-14）：Preview 获焦时 Esc **只在有 Preview 选区时**清除选区并消费，否则继续按规则收起。plan 09 写入完整的
Esc 行为表。与现有实现相比需要两处产品改动，单独提交：透明度弹层打开但数字输入未获焦时 Esc 关闭弹层（现为不消费）；
Preview 获焦且无选区时 Esc 继续收起（现为只清除选区）。其余分支与现有实现一致（Search 字段 composition 中 Esc 由 IME
消费，见 `search_controller.rs`）。

实施分批（按第 3 轮建议）：1a 契约与投影文档；1b Esc 产品改动；2 注册表、上下文、日志、归约、readiness；3 环境采集、
被测实例与记录器；4 观察任务导出、独立记录与整包导入。每批经 gpt-6-astra 审查后提交推送。

### 3-A 第 1a 批文档审查第 1 轮（2026-10-14）

3 个 BLOCKER、7 个 SHOULD_FIX，核实后成立并已修：Esc 第 2 行改为实际的输入守卫（恢复待处理、资产对账进行中、窗口不接受
编辑输入），外部修改冲突不在此列；档案 waiver 写明是限定于 `previous` 的跨 tier 例外，旧"只绑定版本、source 与
case/group"改为引用统一规则；readiness 接受集合加入当前上下文中经归约的人工观察；`ver` 条款限定于 exact evidence；
记录、导入与 readiness 共用匹配谓词并重算角色；M34 例外覆盖开始采集与启动归属，事后采集与人工声明分开；plan 04 的
收起优先级与 AC-022 改为指已送达的窗口 intent 并引用 plan 09 的 Esc 顺序；schema 1 收据保留并报告已被取代；`RESET`
摘要对象为损坏日志字节；被测实例补回保留原 note、不得重新 seed、预期断连不判 FAIL、会话目录清理与有界保留；guided
session 的 `NOT_TESTED` 只出现在会话输出中。

未采纳的修法：审查建议现在就为记录模型添加标为 `NOT TESTED` 的验收行。治理合同只允许 Automated 行为
`AUTOMATED PASS` 或 `BLOCKED`（`governance.rs` 的 mode/status 校验），未实现不等于"已知失败"；因此这些行随第 2–4 批
实现一起加入（预定 P14-A73..A77：目标收据与角色匹配、日志与归约、上下文与恢复、记录器与被测实例、观察任务与导入）。

### 第 1a 批审查第 2 轮与第 1b 批审查第 1 轮（2026-10-14）

1a：PASS / no blocker；3 个 SHOULD_FIX 已修：导出观察任务在共享锁内从同一快照取上下文、`base` 与未解除 FAIL；日志首次创建、
缺失、为空、损坏与达到条数上限的处理（缺失视为丢失，不当作首次初始化；可读日志重置时未解除 FAIL 带入新上下文）；
plan 11 增加不带状态的验证清单，coverage matrix 注明随 3-A 第 2–4 批进入 Phase 14 矩阵。

1b：1 个 BLOCKER——Preview 只把无修饰键的 Esc 识别为 Escape，Shift/Ctrl+Esc 在 Preview 焦点下被吞掉；修正为不论修饰键。
SHOULD_FIX：取消透明度弹层未结束滑块拖动（`ControlState::cancel` 现在一并结束拖动与数字编辑）；原单元断言不保护行为
变化（新增纯函数测试：弹层取消的四种焦点/拖动组合、`preview_escape` 对默认、`caret(17)`、正反向非空选区的判定，以及
修饰键）。NEEDS_VERIFICATION：Split 中焦点移到 Preview 后可能残留 Source preedit；编辑区 composition 的 Esc 判定已移到
输入守卫之后、Search 与 Preview 之前，与 plan 09 的顺序一致。验证：`stickymd-win` 268 个测试通过，clippy 通过。真实桌面
的 IME 事件顺序、修饰键投递、拖动取消与长按穿过多层状态留待覆盖补全批，不宣称已验收。

确认轮（2026-10-14）：1b PASS / no blocker；1a PASS / no blocker，2 个 SHOULD_FIX 已修——目标收据声明的同一事务写入只有头部的
零观察日志，此后缺失、零字节或损坏一律视为丢失；满额重置时未解除 FAIL 按 `(项, 角色, 场景)` 合并带入，保证重置后有追加
容量。真实桌面的 Esc 行为（IME 事件顺序、修饰键投递、拖动取消、长按跨层）仍待覆盖补全批实测。

## 3-A 第 2 批实施记录（2026-10-14）

新模块 `qualification/manual_observation/`：`registry`（24 项、tier、session、逐项场景与已知覆盖缺口；测试保证与
`phase-12.md` 的 Manual 集合相等、14/7/3）、`target`（目标收据严格解析与随机上下文 ID）、`role`（匹配谓词）、`log`
（日志格式、上限与幂等追加）、`reduce`（与登记顺序无关的场景归约，以及重置时的未解除 FAIL 计算，两者共用同一规则）、
`store`（sidecar 锁、带上限读取、声明、重置、合并带入与归档；写入顺序先日志后目标收据）、`waiver`（key 与
`(项, 角色)` 集合的一致性）。`decisions` 的 waiver 记录增加上下文与集合，`--manual-scope` 写入时校验，readiness 读取时
再校验；`RESET-MANUAL-LOG-<digest>` 成为可记录的决定。`manual_readiness` 改为读取上下文、归约、按 tier 与 waiver 处置，
readiness 普通输出与 JSON（schema 3）都包含逐项逐角色汇总、旧格式提示与已知覆盖缺口。成功账本的有界锁等待抽为共享
`file_lock`（列为全局输入）。schema 1 收据模块删除，文件保留并报告已被取代。

过渡状态（如实说明）：观察记录器随第 3 批到来，本批期间 `acceptance manual run/guided` 拒绝执行，而不是继续写已被
取代的收据；`list`、`status`、新增的 `target`（USER 交互声明）与 `reset` 可用。被测机器上的环境事实采集也在第 3 批。

验证：Windows `cargo test -p stickymd-smoke --locked` 全部通过，clippy `-D warnings`、fmt、phase-00 通过；WSL Linux 回放
clippy 通过、356 + 19 通过、phase-00 通过。新增验收行 P14-A73..A75。

### 第 2 批代码审查第 1 轮（2026-10-14）

第一次运行中断、未写报告（按约定重试一次）；中断前指出 reset 先移走活动文件再写新文件，可能被当作首次初始化——已改为
复制归档、活动文件保留到新日志与新目标依次替换，任何仍可解析的日志都携带未解除 FAIL。重试结果：3 个 BLOCKER、6 个
SHOULD_FIX、1 个 NICE_TO_HAVE、1 个 NEEDS_VERIFICATION，核实后全部成立并已修：

- BLOCKER：reset 的未解除 FAIL 计算不验证环境，一条无效 PASS 就能清掉有效 FAIL。现在归约与 reset 共用 `invalidity`；
  reset 只有在日志确实绑定到某个目标时才让有效 PASS 解除 FAIL，否则携带全部 FAIL。
- BLOCKER：决定收据的读改写没有锁，并发更新可恢复已撤销的 waiver；reset 在取锁前读取批准。现在决定收据有独立
  sidecar 锁，所有写入是锁内读改写；锁顺序固定为"人工存储锁先于决定锁"；`declare`/`reset` 在存储排他锁内调用时才
  读取批准；readiness 在两把锁内取得上下文与决定的一致快照。
- BLOCKER：readiness 先读 candidate、人工检查再读一次文件算摘要，可能混用两个 candidate。现在 `read_candidate_snapshot`
  从同一次读取的字节同时给出 candidate 与摘要，readiness 与人工命令都只用它。
- SHOULD_FIX：携带的 FAIL 在目标变化后被判无效、无法解除 → 携带的 FAIL 保留原角色（只能阻断，且日志解析要求携带项
  必为 FAIL），之后看到它的有效 PASS 可以解除。超过字节上限的日志无法授权重置 → 重置摘要改为对文件流式计算，写入
  前检查新日志大小。目标收据接受 `Home`/`Pro` 与不存在的日期 → 拒绝这两个政策标签并校验真实日历日期。readiness
  缺少重置来源、已覆盖场景与"USER WAIVED 只表示符合批准政策"的说明 → 均写入普通输出与 JSON。账本错误会跳过人工
  汇总输出 → 改为先打印账本错误再始终打印人工汇总；无 candidate 时也报告旧收据。过渡状态未同步 → 操作指南加过渡
  说明，P12-A10 与 P14-A12 改写证据，新增"`run/guided` 拒绝且不写文件"的测试。
- NICE_TO_HAVE：批量追加的 ID 去重改为一次建索引（线性）。
- NEEDS_VERIFICATION：新增有界等待超时测试（共享 `file_lock`），以及 Windows 上真实的写入失败注入（目标收据只读，
  替换在日志之后失败）：结果为阻断状态，旧目标收据字节不变，批准后重新声明仍带入 FAIL。

验证：Windows `cargo test -p stickymd-smoke --locked` 432 + 25 通过，clippy、fmt、phase-00 通过。

### 第 2 批代码审查第 2 轮（2026-10-14）

第 1 轮修复经源码核对成立；新增 2 个 BLOCKER、2 个 SHOULD_FIX，核实后已修：

- BLOCKER：readiness 读两次决定（发布门一次、waiver 一次），交错的撤销与批准可产生从未成立过的 READY。现在 readiness 先取
  人工存储共享锁，在锁内只读一次决定，发布门与人工政策共用这一份快照；人工检查不再自行读取决定（回归测试：磁盘上没有
  决定收据时，调用方提供的 waiver 仍生效）。
- BLOCKER：公共证据写入入口可覆盖人工日志、目标收据、归档、sidecar 锁与决定收据。现在 `validate_public_evidence_path`
  经别名解析拒绝这些路径（测试含 `./` 别名与归档子路径，且被拒后日志字节不变）。
- SHOULD_FIX：readiness 收据写入失败会跳过人工汇总 → 先打印汇总再传播写入错误。reset 把决定读取或锁超时误报为"需要
  批准" → 批准回调返回 `Result<bool, String>`，读取或锁错误原样上报（测试）。

验证：Windows 435 + 25 通过，clippy、fmt、phase-00 通过。readiness 写入失败时的输出顺序由代码结构保证，未做输出捕获测试。
Linux 回放发现 `shared_readers_coexist_and_exclude_a_writer` 偶发失败（Unix flock 属于打开的文件描述，其他测试 fork 的子进程
在 exec 前短暂持有）；释放后的获取改为有界重试，锁超时测试释放后的等待放宽到 5 s。Linux 连续三次 364 + 19 通过、
phase-00 通过。

### 第 2 批代码审查第 3 轮（确认轮，2026-10-14）

单快照与锁顺序（三条路径 manual S→decision S、manual S→decision X、manual X→decision S 无环）、错误传播与公共证据入口
的路径保护经核对成立。新增 2 个 BLOCKER、1 个 SHOULD_FIX，已修：

- BLOCKER：release 的 checksums、SBOM、package 发布与 notices 等入口按调用方路径直接写，可覆盖人工存储与决定收据。
  `release::execute` 在分派前对所有调用方指定的输出文件复用 `validate_public_evidence_path`，对输出目录用新的
  `validate_public_output_directory`（拒绝成功账本存储与人工归档）；回归测试覆盖四类入口，被拒后日志字节不变。
- BLOCKER：目标声明在等待 USER 输入前读取 candidate，提交时不再核对；其间 Promote 新 candidate 后，旧声明会把新
  candidate 的上下文按"另一 candidate"直接替换。`store::declare/reset` 现在在排他锁内重读 candidate 摘要，与开始时
  不同即拒绝且不改动任何文件（回归测试）。
- SHOULD_FIX：锁测试的重试循环会把 `TryLockError::Error` 当作成功 → 三种结果显式匹配。

验证：Windows 437 + 25 通过，clippy、fmt、phase-00 通过；Linux 回放见下。
Linux 回放：clippy 通过，连续两次 366 + 19 通过，phase-00 通过。

### 第 2 批代码审查第 4 轮（2026-10-14）

锁内重读 candidate 与锁测试三分支修复成立。1 个 BLOCKER、2 个 SHOULD_FIX、1 个 NICE_TO_HAVE，已修：

- BLOCKER：release 防护按仓库根解释相对路径，写入方按工作目录解释，从子目录调用时检查与写入不是同一文件。防护现在先用
  与写入方相同的 `std::path::absolute` 解析再检查（回归测试：相对工作目录的输出指向存储时被拒）。
- SHOULD_FIX：派生输出（目录内的 ZIP、`SHA256SUMS.txt`、默认 SBOM）可经存储文件处的符号链接改写人工日志。存储读取
  与 sidecar 锁现在拒绝链接（Unix 回归测试），因此派生文件写到哪里都改不到存储读取的内容；派生文件名本身与存储文件名
  不重合。
- SHOULD_FIX：尚未创建的保留文件可被输出操作占成目录（如 `manual.lock/x.json`）。保留文件同时按前缀保护，其下级路径
  一律拒绝；目录检查加入决定收据与其锁。
- NICE_TO_HAVE：防护分派去掉通配分支，逐一列出无调用方输出的 release 命令，新增命令必须显式归类。

验证：Windows 438 + 25 通过，clippy、fmt、phase-00 通过；Linux 回放 clippy 通过，连续两次 368 + 19 通过，phase-00 通过。

### 第 2 批代码审查第 5 轮（2026-10-14）

PASS / no blocker；第 4 轮其余修复成立（release 防护与写入方统一按工作目录解析、存储与 sidecar 拒绝链接、release
分派穷举）。1 个 SHOULD_FIX 已按审查的最小修法修复：`manual.lock/../diagnostic.json` 在检查时折叠为普通路径，写入时
`create_dir_all` 却会先把 `manual.lock` 建成目录。保留路径检查改为 `path_identity::reaches`：原始路径的每一级祖先在
别名解析后、`..` 折叠前与保留路径比较；回归测试覆盖存储文件尚不存在的情形。

最终验证：Windows `cargo test -p stickymd-smoke --locked` 439 + 25 通过，clippy `-D warnings`、fmt、phase-00 通过；WSL Linux
回放 clippy 通过，连续两次 369 + 19 通过，phase-00 通过。未覆盖：Windows 原生 junction/8.3 实测、真实并发 Promote、
readiness 写入失败时的输出捕获、性能与内存测量。
