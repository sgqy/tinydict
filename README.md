# TinyDict

使用 Rust 和 egui 制作的本机英语词典。CloudBytes、Wordset、Simple English Wiktionary 和英语 Wiktionary 分别存储、查询和展示。默认在后台逐本完整加载；低内存模式将全部词头索引留在内存，正文从磁盘上的 zstd 压缩块按需读取。

## 启动

需要 Rust 1.100 或更新版本、Windows C++ 构建工具和 Python 3。首次准备需要联网下载公开词典与 Rust 依赖，日常使用完全离线。

在项目目录运行：

```powershell
./scripts/setup.ps1
./dist/tinydict.exe
```

已下载全部来源时使用 `./scripts/setup.ps1 -SkipDownload`。脚本默认生成 `.tdict.zst` 压缩包；已有 SQLite 包会转换并逐条校验，旧包保留。已有压缩包不自动覆盖。`dist` 包含程序与词典，可整体移动到本机其他目录。

也可从 [数据集预发布](https://github.com/sgqy/tinydict/releases/tag/datasets-2026.11.17-preview.1) 下载四个 `.tdict.zst` 文件，放入 `data/dictionaries`，再运行 `./scripts/setup.ps1 -SkipDownload`。这样无需下载原始数据。该发布仅上传数据包，程序从源码构建。

开发时运行：

```powershell
cargo run --release -p tinydict -- --data-dir data/dictionaries
cargo run --release -p tinydict --bin tinydict-inspect -- --lookup computer
cargo run --release -p tinydict --bin tinydict-inspect -- --benchmark
```

内存较少的机器可加 `--low-memory`：

```powershell
./dist/tinydict.exe --low-memory
```

这个开关无需填写内存数值，也不限制线程数。首次启动会在词典目录的 `.tinydict-cache` 中生成分块压缩缓存，后续启动复用缓存。该目录需要写入权限。四本当前词典的缓存额外占用约 314.5 MiB；原始数据包保留。更新数据包后按 SHA-256 创建新缓存，旧缓存可在程序关闭后删除。

Windows 默认使用 Direct3D 12，macOS 使用 Metal，其他系统使用 Vulkan。图形后端在独立进程中启动，初始化失败、崩溃或 30 s 内未报告就绪时尝试 OpenGL。`--renderer vulkan` 可优先尝试 Vulkan，失败后继续尝试平台后端；`--renderer glow` 可优先尝试 OpenGL。

## 使用

- 输入单词或词组即可查询。左侧候选保留词典来源，右侧逐本显示释义。
- 左侧复选框控制参与查询的词典。停用查询不会卸载已加载数据。
- `Ctrl+L` 聚焦搜索框，上下键选择候选，回车打开。
- 释义支持文本选择、复制、同义词跳转。长词条分批显示，每次增加 24 个义项。
- 每个义项先显示最多 3 条例句，点击 `Show more examples` 继续展开。低内存模式只读取命中词头的正文，保留全部例句。Wiktionary 引文保留出处，原词条链接可查看贡献者历史。
- `Save word` 保存当前单词；清空搜索框后可查看收藏与查询历史。
- `Sources & licenses` 显示原始来源、版本和数据授权信息。
- 主题、词典开关、历史与收藏由 eframe 保存到本机用户配置目录。

## 数据和来源

| 词典 | 来源版本 | 原始条目 | 义项 | 规范化词头 | 内存数据估算 |
|---|---|---:|---:|---:|---:|
| CloudBytes | `ef5562840cf57c7682fc63e7e35ddb616638f93b` | 388,587 | 388,587 | 221,507 | 197.8 MiB |
| Wordset | `4503bfafbe4556361a296b9d368c18168dfb56c3` | 108,128 | 163,274 | 107,946 | 75.3 MiB |
| Simple English Wiktionary | Kaikki 2026-10-30 / SHA-256 `79afd871e342…` | 61,743 | 73,204 | 53,891 | 37.7 MiB |
| English Wiktionary | Kaikki 2026-10-30 / SHA-256 `4d03b6cba283…` | 1,497,827 | 1,793,712 | 1,360,430 | 1,193.7 MiB |

估算包含结构化条目、字符串和 FST，不等于进程总内存。导入器统计实际文件，后续来源版本的数据量可能变化。

Simple English Wiktionary 保留了 25,492 条例句；英语 Wiktionary 保留了 770,517 条例句和引文。例句按所属义项展示，不跨来源补写或合并。英语 Wiktionary 的词头包含词形、专名和罕见词，不代表同等数量的日常词汇。来源比较与后续候选见 [词典收集清单](docs/dictionary-sources.md)。

CloudBytes v2 自身包含 Webster 1913 和 Open English WordNet。本程序将 CloudBytes 作为一本词典，保留每个义项的 `source`、原始词性和 `synset_id`。Wordset 保留义项顺序、例句、同义词、标签和贡献者。不同来源之间不合并、不去重。

Wordset 当前数据有 374 个空同义词项，导入时忽略；另有 5 条缺失义项的记录，导入报告将其计为跳过。无法解析的 JSON 会使导入失败，避免静默生成不完整词典。

Wiktionary 导入只接受 `lang_code=en` 的词条。Simple English 快照跳过 496 条重定向和 8 条无释义记录；英语版跳过 1,334 条无释义记录，共剔除 1,335 个空义项。空例句不展示。原始压缩文件保留完整数据，当前词典包仅提取释义、例句、引文出处、义项同义词和用法标签。

下载脚本记录上游提交和每个文件的 SHA-256。再次运行会复用固定版本，并验证已有文件。原始数据保存在 `data/sources`，词典包保存在 `data/dictionaries`。两者均排除在 Git 之外。

`python scripts/fetch_wiktionary.py` 下载两本 Wiktionary，记录 HTTP 版本标记与 SHA-256。首次运行获取当时的快照，之后仅验证本地固定文件。英语版使用 Kaikki 的后处理英语切片；该下载渠道已被上游标记为弃用，后续更新需要迁移到原始提取数据。

数据条款见 [CloudBytes LICENSE-DATA](https://github.com/CloudBytes-Academy/English-Dictionary-Open-Source/blob/main/v2/LICENSE-DATA.md) 和 [Wordset LICENSE](https://github.com/wordset/wordset-dictionary/blob/master/LICENSE)。每个包内保存对应版本的原始许可证，代码与数据的授权分别处理。

两本 Wiktionary 使用 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)。包内保存许可正文、Wiktionary 贡献者署名、Kaikki 提取来源和转换说明；引文另保留上游提供的出处。

## 许可证

本项目原创代码和文档采用 [MIT 许可证](LICENSE)。第三方依赖、词典数据及其衍生词典包遵循各自的许可证，项目的 MIT 许可证不替代这些条款。

Git 仓库包含源码、脚本和文档。词典包通过数据集预发布分发，发布说明列出各来源协议；原始下载数据和构建产物不提交到 Git。

## 结构和性能

| 模块 | 职责 |
|---|---|
| `crates/core` | Unicode 规范化、独立词典模型、全内存 FST 查询、资源预算 |
| `crates/data` | 来源适配器、输入校验、词典包、全量加载与压缩块磁盘查询 |
| `crates/app` | egui 界面、图形启动回退、后台加载、最新查询调度、命令行检查与基准 |

默认存盘格式 `.tdict.zst` 使用 zstd 压缩 JSONL。首行保存版本、来源、许可证、输入哈希和导入报告，后续每行保存一个条目。全内存模式先读元数据判断内存预算，再流式解压并建立 FST；不生成临时解压文件，不缓存完整解压文本。

旧版 `.tdict` SQLite 文件仍可加载。同一目录同时存在 `book.tdict` 和 `book.tdict.zst` 时，只加载压缩版。四本词典的文件总量从 1,118.5 MiB 降为 162.1 MiB，减少约 85.5%。全内存模式的常驻数据估算为 1,504.5 MiB（1.47 GiB），低内存模式的常驻索引约为 81.4 MiB。格式、校验和转换命令见 [词典包格式](docs/package-format.md)。

查询线程与界面通过消息通信。待处理请求仅保留最新一条，结果带版本编号，旧结果不能覆盖新输入。全内存模式的结果引用共享只读词典；低内存模式由查询线程读取、解压命中词头，界面保留当前结果。磁盘访问不在界面线程执行。候选列表按可见行绘制；空闲时按事件重绘。

导入器按可用逻辑处理器数设置并行度，最多 22 个工作线程。Wordset 按字母文件并行解析；CloudBytes 和 Wiktionary 的单个压缩 JSONL 顺序解析。zstd 使用级别 6 压缩，最多 22 个压缩线程；旧版 SQLite 包使用单写入事务。多本词典的内存查询使用一个后台线程，避免微小任务的跨线程调度成本。

加载时根据总内存、当前可用内存和预计展开体积判断是否接纳词典。4 GB 档位的数据预算上限为 768 MiB，96 GB 档位为 64 GiB；这不是预分配。还会为系统和加载过程预留内存。内存不足时明确报告该词典未加载，其他已加载词典仍可查询。

完整英语 Wiktionary 的全文超过 4 GB 档位的数据预算，全内存模式会拒绝加载。使用 `--low-memory` 后按索引估算接纳词典，当前四本索引可落在该预算内，仍需满足系统可用内存检查。默认保持全量加载。

本版通过启动参数选择低内存模式，尚未实现运行中自动切换、卸载、拼写纠错、全文检索或 CUDA。普通查词使用 CPU。界面保留垂直同步和 1 帧的目标排队上限。跨平台代码已保留，本次交付验证范围为 Windows。

早期 OpenGL 长帧促使默认后端改用 Vulkan。收到 Intel UHD 630 + RDP 环境的启动崩溃反馈后，默认策略改为平台后端与独立进程回退。启动日志位于 Windows 的 `%APPDATA%\TinyDict\data\graphics-startup.log`。就绪后发生的驱动崩溃会记录退出状态，不自动重启。测量过程和局限见 [窗口性能诊断](docs/frame-performance.md)。

## 验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

测试覆盖来源隔离、大小写冲突、Unicode、词组、前缀限额、缺失字段、压缩输入、包往返、关闭并删除包后继续查询，以及快速输入时的最新请求处理。

Wiktionary 测试另覆盖非英语筛选、重定向计数、层级释义、引文出处、空例句过滤，以及错误行号。压缩包测试覆盖旧格式兼容、截断、校验失败、数量不符、超大记录、失败清理和新旧包共存选择。低内存测试覆盖正文一致性、缓存复用、来源更新、磁盘缺失和损坏；图形测试覆盖子进程异常退出、超时回收与就绪握手。当前工作区执行 25 项测试（包选择测试在两个程序中各执行一次）。

`tinydict-inspect --benchmark` 加载所有词典后执行 20,000 次联合精确查询与前缀查询，报告 p50、p95 和 p99。该结果只衡量热内存查询，不包含首次加载、界面排版和输入延迟。桌面程序使用 Windows GUI 入口，双击启动时不打开额外的控制台。

2026-11-17 在当前 Windows 开发机上的一次 release 实测：两本词典加载用时 1.345 s，联合精确查询 p95 为 0.002 ms，前缀查询 p95 为 0.016 ms（每本最多 50 个候选）。这些是当前数据快照与测试进程的结果，不代表其他机器或冷启动性能。

同日扩展到四本词典后，控制台加载用时 7.531 s，联合精确查询 p95 为 0.005 ms，前缀查询 p95 为 0.035 ms。原生窗口查询 `run`、持续重绘并注入指针移动，20 s 采集 2,400 帧：界面构建 p95 为 0.602 ms，帧间隔 p95 为 8.622 ms，最大值为 10.345 ms。结果保存在本机 `data/reports`，测量限制与原窗口诊断相同。

实际窗口已验证四本词典独立展示、词典开关和同义词跳转，以及新增词典的例句、引文出处和来源入口。4 GB 环境下的系统换页及端到端输入延迟尚未实测。

改用 zstd 后，同机一次 release 实测加载四本用时 6.481 s，精确查询 p95 为 0.005 ms，前缀查询 p95 为 0.044 ms。转换工具已对全部条目、义项、例句、来源和导入报告逐一核对。这次测量不代表冷启动性能，也不能据此判断两种格式的稳定速度差异。

低内存模式在同一开发机上首次建缓存用时 18.511 s，复用缓存加载用时 1.284 s。四本常驻索引估算为 81.4 MiB，首次建缓存的控制台进程峰值工作集约为 141.8 MiB。复用缓存后抽样读取 1,004 个词头，正文读取 p95 为 0.337 ms；这些是本机文件缓存已热的结果，不代表低性能机或冷盘速度。两种模式查询 `run` 的四本释义和例句输出一致。

本次已验证开发机 Direct3D 12 渲染就绪，并通过全部自动检查。Intel UHD 630 + RDP 仍待目标机器复测。明亮模式已修正透明面板和黑色清屏底色；桌面访问受限，尚未完成修改后的主题截图复核。

## 窗口帧耗时诊断

使用 release 程序对比渲染后端：

```powershell
./dist/tinydict.exe --renderer glow --profile data/profiles/glow.json --profile-motion --profile-seconds 15
./dist/tinydict.exe --renderer vulkan --profile data/profiles/vulkan.json --profile-motion --profile-seconds 15
```

两条命令应依次运行。保持测试窗口在前台；默认测空白首页，添加 `--query computer` 可测查询结果页。添加 `--no-vsync` 可比较关闭垂直同步后的结果。

测试窗口需要实际可见。使用 PowerShell 自动启动时，应使用 `Start-Process -WindowStyle Normal`，并确认窗口未被遮挡。隐藏启动的测试曾出现异常限帧，即使报告中的焦点标记为真，也不能据此认定窗口在前台。

诊断等待词典加载和查询完成，预热 1 s 后采样，结束时自动关闭窗口。JSON 分别记录界面构建时间、eframe 帧 CPU 时间和帧间隔，并单独汇总连续两帧均有焦点的样本。eframe 的 CPU 计时扣除了其识别的呈现等待；帧间隔还包含事件循环、调度和呈现等待。同名 `.log` 文件记录实际显卡与后端。

`--profile-motion` 向 egui 注入可重复的指针位置，诊断模式持续重绘。它用于检查悬停和渲染开销，不模拟操作系统的高回报率鼠标队列，也不测量实际鼠标到屏幕的延迟。测试配置写入旁边的 `.state.ron`，与日常配置隔离。正常启动不采样，也不持续重绘。
