# Phase 17 macOS 虚拟机验收（2026-11-18）

Phase 17 的 macOS 人工视觉项（P17-N12、P17-R06）与资源项（P17-C12）在 `mac-test` 虚拟机里的记录。
行定义见 [Phase 17 验收矩阵](../acceptance-cases/phase-17.md)，实施进度见 [Phase 17 任务](../tasks/phase-17-copy-and-notices.md)。

## 身份

- **被测程序：** `2d73ab9` 的 release，用 `apps/stickymd-mac/packaging/build-app.sh` 打包、ad-hoc 签名。
  `b4ccd6b` 之后只改了文档，产品代码与分支 CI `44113614349` 测过的 `b4ccd6b` 相同。
  可执行文件 SHA-256 `806c03ec66e36df51175d6295de5095a3ce73251815714152e55834eda4102e4`。
- **资源测量的基线：** Phase 17 分支的起点 `0841524`，在单独的工作树里用同一脚本打包。
  可执行文件 SHA-256 `bf1ec9c0c916148633516feaf99e492141312554e4489868334faeb6681dc1ed`。
- **驱动：** `2d73ab9` 的 `stickymd-smoke`，按 `CARGO_PROFILE_RELEASE_PANIC=unwind CARGO_TARGET_DIR=target/unwind cargo build --locked --release -p stickymd-smoke` 构建，
  SHA-256 `025f1424a02a5a8bdc698a21cb05c8cef1fe1b19d7dca168ea1c7752f3e32ad7`。
- **虚拟机：** `mac-test`，macOS 27.2（26B123），`tart run --no-graphics`，经 SSH 主连接操作。

## 人工收据：P17-N12、P17-R06（macOS，2026-11-18）

**做法：**

- 被测程序装在虚拟机的 `~/p17`，程序目录里放一份测试笔记。笔记包含：
  - 短代码块、长行代码块（带横向滚动条）、空代码块；
  - 两个块级公式（多行与单行 `$$`）、一个 Raw HTML 块；
  - 引用中的代码块，以及四层列表中的代码块；
  - 一张表格与一个行内公式，二者不应有侧条。
- USER 经“屏幕共享”连到虚拟机，自己操作并查看。
- 给 USER 的检查清单：
  - R06：
    1. 侧条的位置与高度，长行代码块的侧条不含横向滚动条轨道，行内公式与表格没有侧条；
    2. 宽度 8 内容 DIP 是否合适；
    3. Light/Dark 下的低对比；
    4. 悬停变亮与手形光标，点击后的“已复制”，空代码块点击后剪贴板不变、无提示；
    5. 40% 透明度；
    6. 放大内容与窄窗下，放不下的侧条整根不画。
  - N12：
    1. Cmd+S 后的“已保存”、点击侧条后的“已复制”，看位置与对比度，Split 下横跨两栏；
    2. 三种外观各看一次：Light、Dark、40% 透明度；
    3. Source 的最后一行可被瞬时提示暂时盖住。

**USER 回复：** “都通过”（2026-11-18）。

**这次没有覆盖的：**

- **两类提示没有触发：** 冲突的待决定提示（带 F6/F7 按钮）与保存失败的故障展示，要由我在虚拟机里改动笔记或目录权限才会出现。
  我提出可以配合触发，USER 没有要求，回复里也没有提到这两类，所以它们的外观不在这次确认之内。
  它们的行为由自动化行覆盖（N07、N11、N13、N16 等）。
- **Windows 宿主：** 没有做。两行的 Windows 部分仍待在 Windows 宿主上确认。
- **侧条宽度：** 8 内容 DIP 在 macOS 上通过。plan 06 写的是“视觉验收后固定”；Windows 的视觉验收还没做，所以 plan 的“暂定”暂不改。

**验收表：** 两行的 Mode 都是 `Manual`，覆盖两个宿主，而 Windows 部分未做，所以 Status 保持 `NOT TESTED`。
Evidence 列加上本收据（macOS 部分）。

## P17-C12：进程内存门（macOS）

**结论：**

- **门：** 两个版本在 8 轮里每场景各 20 次运行全部有效，最大 `phys_footprint` 都在 plan 10 macOS 临时硬门内，余量至少 16 MiB。
- **与基线比：** 按缓冲档位分层后，被测程序比基线 `0841524` 高约 0.1 MiB。这与同一版本两轮之间的差同量级，每个版本只有两轮，分不开。完整分层里至多 +0.15 MiB，稀疏分层最高 +0.24 MiB。
- **验收表：** C12 仍为 `NOT TESTED`，因为 Windows 资源运行没有做。

**做法：**

- 驱动与两个程序见“身份”一节。夹具是 `mac-resources` 自带的 20 KiB、20 个公式（SHA-256 `1f8a0ae3bc20badbc13e602b77ea5aa2992c3ecbb4791c33f2887876c509edb6`）。
- 每轮每场景 5 次，预热 30 s、空闲 60 s；按 Source、Preview、Split、Hidden 的顺序，每个场景连续跑 5 次（按 PID 顺序核实）。
- 两批，时间为虚拟机时钟（UTC，即北京时间 11-18 凌晨）：
  - 批次 1，16:06–18:09，顺序基线、被测、被测、基线（r1–r4）；
  - 批次 2，18:24–20:27，顺序被测、基线、基线、被测（r5–r8）。
- 两批之间（18:13–18:24）做了 9 对背靠背探针：同一场景先后启动两个版本，30 s 后取 `footprint` 与 `vmmap --summary`。夹具哈希与上面相同。
- **运行条件：**
  - 已核实：视觉验收的实例在批次前停掉（16:05 `pkill` 后 `pgrep` 无结果）；屏幕共享 16:04:24 断开；r1 在 16:06:01 开始。
  - 已核实：`pmset -g` 为 `sleep 0`、`displaysleep 0`。
  - 已核实：统一日志里，16:06–20:28 的 `powerd`、`loginwindow`、`screensharingd`、`WindowServer` 没有睡眠、熄屏或屏幕共享连接事件。
    每小时一次的 darkwake 查询只持有 20 s 的断言，没有进入睡眠。
  - `powerd` 在 16:09:21 随用户活动级别变化记下 `Device is locked/inactive`，之后没有变化。这条字样不区分锁屏与无人操作。
    推断不是锁屏：8 轮的 40 次 Hidden 运行都经菜单栏隐藏（`hidden_through_menu_bar`），全部有效。
  - JSON 里的 `release_build`、`desktop_conditions` 由调用方声明，驱动不核实，以上即本次的核实。
    命令与匹配到的日志行存于证据目录的 `analysis/run-conditions.txt`；原始日志摘录没有保留。

**门（每版本每场景 20 次，单位 MiB）：**

| 场景 | 临时硬门 | 被测 中位/最大 | 基线 中位/最大 |
| --- | --- | --- | --- |
| Source | > 72 | 40.90 / 46.53 | 36.82 / 46.58 |
| Preview | > 96 | 45.20 / 49.33 | 45.03 / 50.55 |
| Split | > 128 | 45.66 / 52.69 | 45.37 / 45.58 |
| Hidden | > 48 | 29.03 / 31.41 | 26.98 / 30.96 |

- 合并的中位含下文的 +4.2 MiB 状态，被测落在其中的次数多于基线，所以这张表只用于判门，不用于比较版本。
- 驱动对每轮都给出 `status` `SCENARIOS_PASS`、`acceptance_status` `INCOMPLETE`。后者是驱动固定写入的：CPU 与唤醒只作观测（`tools/stickymd-smoke/src/mac_resources/report.rs:93`、`:139`）。
- 空闲 CPU 每次运行最大 0.0025%，中断唤醒最大 0.067/s，只作观测。

**缓冲档位：**

- 呈现器的每个 IOSurface 缓冲约 5.5 MiB（窗口 520×680，scale 2），运行中有 2 个或 3 个，`footprint` 因此分成相差约 5.5 MiB 的档。
- 探针的 `vmmap` 只见过 2 个（11.0M）与 3 个（16.5M）两档；驱动运行里的档位是按数值推断的。
- 第三个缓冲出现的次数（被测对基线，各 20 次）：Source 5 对 5，Preview 14 对 12，Split 15 对 16，Hidden 都是 0。分层不会掩盖档位比例的差别。

**+4.2 MiB 状态（原因未查明）：**

- 所有场景同时抬高约 4.2 MiB，与缓冲档位叠加。
- 开始于 r1（基线）Hidden 第 2 次，约 16:30；结束于 r4（基线）Preview 第 2 次，约 17:47。时刻按 PID 顺序与平均每次 92 s 推算。中间是 r2、r3（被测）。
- 窗口内有 10 次基线运行（r1 Hidden 4 次，r4 Source 5 次、Preview 1 次）；批次 2 的 80 次运行里没有出现。起止都落在基线轮次，所以不是被测程序带来的。
- 上述日志里这段时间没有对应事件；探针在状态消失之后才跑，没有这一状态的 `vmmap` 记录。

**版本比较（主：批次 2，无 +4.2 状态）：**

每格是该轮该档位运行的中位（MiB）；被测 − 基线 = 两轮被测的均值减两轮基线的均值，被测、基线、基线、被测的顺序抵消线性漂移。

| 场景、档位 | 被测 r5、r8 | 基线 r6、r7 | 被测 − 基线 | 同版本两轮之差（被测 r8−r5、基线 r7−r6） |
| --- | --- | --- | --- | --- |
| Source，2 个缓冲 | 36.87、36.98 | 36.80、36.77 | +0.14 | +0.11、−0.03 |
| Preview，3 个缓冲 | 45.13、45.25 | 45.13、45.06 | +0.09 | +0.13、−0.06 |
| Split，3 个缓冲 | 45.53、45.63 | 45.38、45.49 | +0.15 | +0.09、+0.11 |
| Hidden | 26.99、27.16 | 26.83、27.02 | +0.15 | +0.17、+0.19 |

- 四个完整分层里被测都高 0.09–0.15 MiB，符号一致；同版本两轮之差为 −0.06 至 +0.19 MiB，同一量级。
- 稀疏分层（某版本只有一轮或每轮 1–2 次）：Source 3 个缓冲 +0.24，Preview 2 个缓冲 −0.02，Split 2 个缓冲 −0.05。
- 探针的 `vmmap` 中，除 IOSurface 的缓冲个数外，两个版本只有 Malloc 脏页有差别：Malloc Small 被测 − 基线为 −0.10 至 +0.20 MiB，Malloc Tiny 不超过 32 KiB；Untagged 相同。
- **来源未定位。** Hidden 也高约 0.15 MiB；按 plan 10，隐藏时释放文档投影，所以这部分不能都归于复制映射（20 KiB 文档 58 KiB）。

**辅助检查（事后分层，8 轮）：**

- 把 8 轮按缓冲档位与 +4.2 状态两个因素分层。4.2 这个常数与各档基准取自同一批数据，所以只作一致性检查。
- 在两个版本都有的 7 个非 +4.2 分层里，被测 − 基线为 +0.02 至 +0.16 MiB，与批次 2 一致；+4.2 分层里为 −0.16 至 +0.17 MiB。
- **两次不在档位上的单次运行，原因未查明：**
  - r1 基线 Preview 第 1 次，50.55 MiB，比 3 个缓冲档高 5.45 MiB，像第 4 个缓冲，但探针从未见过 4 个；
  - r2 被测 Split 第 4 次，52.69 MiB，离最近的档位 1.7 MiB。
  - Phase 16 方案 B 的反序批次也出现过这样的孤立最大值：Preview 50.5、Split 48.0 MiB（`docs/acceptance-cases/phase-16.md:77`，P16-R01 的 macOS 内存基线）。
- **局限：** 每个版本只用一次构建，没有按构建分组。

**证据**（已忽略的 `dist/evidence/mac-vm-2026-11-18/`）：

- `p17-c12/`：`out/r*/mac-resources.json`、`log-r*.txt`，以及 `run.sh`、`run2.sh`、`progress.txt`、`progress2.txt`。各轮 JSON 的 SHA-256：
  - r1 基线 `0bdaf23b6dadcbeb30020052ec769ad43bfae32b84659f36c36e002fcdee9800`
  - r2 被测 `fba0fa41859a548e04ea1a5e12f390cacaed4bbe1f0906129540f8ae7aece82c`
  - r3 被测 `31a40ba3f6cf71b447f52edad5fc38bae10cd1af4cbed97b9259095410b1789d`
  - r4 基线 `e90a4c1963e993b97a39c7a7425897f32fad3e4434493b2b9cc48b885c90d97a`
  - r5 被测 `de137dcad204a94283e8a81b62a04c7ecf3b1d3ca2255e9e4c99a11d522dd852`
  - r6 基线 `93c3608dc68d8e3c153f1dc8d7aea4a7b78813de9f2da6b4a209260a943642f9`
  - r7 基线 `8cce706307820de09c07fcb15c917016a1ca2692902c8329b97fcedbb895fbd3`
  - r8 被测 `e54fc3f6817c8763df64481e3e15a1e8ef0c6d7d8dea66d92b5e3887ac81b3ee`
- `p17-c12/analysis/`：本节表格所用的脚本 `c12_all.py`（逐轮汇总）、`c12_strata.py`（8 轮两因素分层）、`c12_batch2.py`（第三缓冲次数与批次 2 分层），以及 `run-conditions.txt`。
- `p17-c12-probe/`：每对探针的 `footprint` 与 `vmmap` 输出，以及 `probe.sh`、`pairs.sh`、`fixture.md`。汇总 `summary.txt` 的 SHA-256 为 `e09a88048b08f8c02472c83d86652351fa5dc460725c4804a99217a69f6e16a1`。

**验收表：**

- C12 的 Mode 是 `Automated`，覆盖两个宿主。macOS 的进程内存门已通过；Windows 资源运行没有做，所以 Status 保持 `NOT TESTED`。
- Evidence 列补上 macOS 部分。
