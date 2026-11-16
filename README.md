# TinyDict

使用 Rust 和 egui 制作的本机英语词典。CloudBytes 与 Wordset 分别存储、查询和展示。词典在后台逐本完整加载；加载完成后，查词不访问磁盘，也不解析 JSON。

## 启动

需要 Rust 1.100 或更新版本、Windows C++ 构建工具和 Python 3。首次准备需要联网下载公开词典与 Rust 依赖，日常使用完全离线。

在项目目录运行：

```powershell
./scripts/setup.ps1
./dist/tinydict.exe
```

已有原始数据时使用 `./scripts/setup.ps1 -SkipDownload`。脚本保留已有词典包，不自动覆盖。`dist` 包含程序与词典，可整体移动到本机其他目录。

开发时运行：

```powershell
cargo run --release -p tinydict -- --data-dir data/dictionaries
cargo run --release -p tinydict --bin tinydict-inspect -- --lookup computer
cargo run --release -p tinydict --bin tinydict-inspect -- --benchmark
```

## 使用

- 输入单词或词组即可查询。左侧候选保留词典来源，右侧逐本显示释义。
- 左侧复选框控制参与查询的词典。停用查询不会卸载已加载数据。
- `Ctrl+L` 聚焦搜索框，上下键选择候选，回车打开。
- 释义支持文本选择、复制、同义词跳转。长词条分批显示，每次增加 24 个义项。
- `Save word` 保存当前单词；清空搜索框后可查看收藏与查询历史。
- `Sources & licenses` 显示原始来源、版本和数据授权信息。
- 主题、词典开关、历史与收藏由 eframe 保存到本机用户配置目录。

## 数据和来源

| 词典 | 来源版本 | 原始条目 | 义项 | 规范化词头 | 内存数据估算 |
|---|---|---:|---:|---:|---:|
| CloudBytes | `ef5562840cf57c7682fc63e7e35ddb616638f93b` | 388,587 | 388,587 | 221,507 | 197.8 MiB |
| Wordset | `4503bfafbe4556361a296b9d368c18168dfb56c3` | 108,128 | 163,274 | 107,946 | 75.3 MiB |

估算包含结构化条目、字符串和 FST，不等于进程总内存。导入器统计实际文件，后续来源版本的数据量可能变化。

CloudBytes v2 自身包含 Webster 1913 和 Open English WordNet。本程序将 CloudBytes 作为一本词典，保留每个义项的 `source`、原始词性和 `synset_id`。Wordset 保留义项顺序、例句、同义词、标签和贡献者。不同来源之间不合并、不去重。

Wordset 当前数据有 374 个空同义词项，导入时忽略；另有 5 条缺失义项的记录，导入报告将其计为跳过。无法解析的 JSON 会使导入失败，避免静默生成不完整词典。

下载脚本记录上游提交和每个文件的 SHA-256。再次运行会复用固定版本，并验证已有文件。原始数据保存在 `data/sources`，词典包保存在 `data/dictionaries`。两者均排除在 Git 之外。

数据条款见 [CloudBytes LICENSE-DATA](https://github.com/CloudBytes-Academy/English-Dictionary-Open-Source/blob/main/v2/LICENSE-DATA.md) 和 [Wordset LICENSE](https://github.com/wordset/wordset-dictionary/blob/master/LICENSE)。每个包内保存对应版本的原始许可证，代码与数据的授权分别处理。

## 结构和性能

| 模块 | 职责 |
|---|---|
| `crates/core` | Unicode 规范化、独立词典模型、全内存 FST 查询、资源预算 |
| `crates/data` | 来源适配器、输入校验、SQLite 词典包、全量加载 |
| `crates/app` | egui 界面、后台加载、最新查询调度、命令行检查与基准 |

`.tdict` 是带版本信息的 SQLite 文件，保存全部原始条目和导入报告。启动时读入并解析所有条目，再建立 FST；数据库连接随后关闭。加载完成前压缩容器的多余容量，两本词典合计约 273.1 MiB 的数据估算适合全量常驻，因此当前包格式不另存 FST 文件。

查询线程与界面通过消息通信。待处理请求仅保留最新一条，结果带版本编号，旧结果不能覆盖新输入。查询结果只引用共享只读词典，避免复制释义。候选列表按可见行绘制；空闲时按事件重绘。

导入器按可用逻辑处理器数设置并行度，最多 22 个工作线程。Wordset 按字母文件并行解析；CloudBytes 的单个压缩 JSONL 顺序解析。每个 SQLite 包使用单写入事务。两本词典的内存查询使用一个后台线程，避免微小任务的跨线程调度成本。

加载时根据总内存、当前可用内存和预计展开体积判断是否接纳词典。4 GB 档位的数据预算上限为 768 MiB，96 GB 档位为 64 GiB；这不是预分配。还会为系统和加载过程预留内存。内存不足时明确报告该词典未加载，其他已加载词典仍可查询。

本版尚未实现内存不足时的磁盘查询降级、运行中卸载、拼写纠错、全文检索或 CUDA。普通查词使用 CPU，界面使用 OpenGL 图形后端。跨平台代码已保留，本次交付验证范围为 Windows。

## 验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

测试覆盖来源隔离、大小写冲突、Unicode、词组、前缀限额、缺失字段、压缩输入、包往返、关闭并删除包后继续查询，以及快速输入时的最新请求处理。

`tinydict-inspect --benchmark` 加载所有词典后执行 20,000 次联合精确查询与前缀查询，报告 p50、p95 和 p99。该结果只衡量热内存查询，不包含首次加载、界面排版和输入延迟。桌面程序使用 Windows GUI 入口，双击启动时不打开额外的控制台。

2026-11-17 在当前 Windows 开发机上的一次 release 实测：两本词典加载用时 1.345 s，联合精确查询 p95 为 0.002 ms，前缀查询 p95 为 0.016 ms（每本最多 50 个候选）。这些是当前数据快照与测试进程的结果，不代表其他机器或冷启动性能。

实际窗口已验证两本词典独立展示、停用其中一本和同义词跳转。4 GB 环境下的系统换页及端到端输入延迟尚未实测。
