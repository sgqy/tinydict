# 词典包格式

## 默认格式

`.tdict.zst` 是标准 zstd 压缩流，解压后为 UTF-8 JSONL。第一行是 `schema_version=2` 的 `PackageManifest`，后续每行是一个 `Entry`，每行以换行符结束。条目字段定义见 `crates/core/src/entry.rs`。

清单保留独立词典身份、来源地址、版本、许可证、条目数、义项数、内存估算和导入报告。义项内的例句、引文出处、标签和来源信息保留原值。词典之间不合并数据。

写入使用 zstd 级别 6、8 MiB 历史窗口和内容校验和。压缩线程数根据可用逻辑处理器调整，最多 22 个。序列化逐条进行，不先构造完整 JSONL 文件。压缩结果先写同目录临时文件，完成后发布到目标路径；已有目标不会被覆盖。

## 读取与校验

读取器根据文件头识别 zstd 或旧版 SQLite 格式。元数据检查只解码首行附近的数据，用于在加载正文前判断内存预算；它不保证整个文件完整。全量加载会读到流尾，检查解压错误、校验和及条目和义项数量，然后建立内存索引。

解压历史窗口限制为 8 MiB，每行解压后最多 16 MiB。读取器复用行缓冲区，不按未经验证的条目数量预分配数组。加载结束后关闭文件；后续查询不访问磁盘或解压器。

`.tdict` 继续表示 `schema_version=1` 的 SQLite 包。同目录同名新旧包并存时，程序选择 `.tdict.zst`。压缩包损坏会明确报错，不静默切换到可能过期的旧包。不同文件名使用同一词典身份时，桌面加载器仍会报告身份重复。

## 转换现有词典

在项目目录运行：

```powershell
cargo run --release -p tinydict-data --bin tinydict-pack -- --input data/dictionaries/wordset.tdict --output data/dictionaries/wordset.tdict.zst --verify
```

`--verify` 重新加载输出文件，逐条比较词条、义项、例句和来源信息，同时比较词典元数据与导入报告。转换期间输入与输出词典可能同时驻留内存。需要恢复 SQLite 格式时，将输入设为压缩包、输出设为新的 `.tdict` 路径。

`tinydict-import` 根据输出名称选择格式：以 `.tdict.zst` 结尾时生成压缩包，其余沿用旧版 SQLite 格式。`scripts/setup.ps1` 默认使用压缩格式，并保留已有旧包。

## 独立读取数据

此格式不要求 TinyDict 才能访问内容。使用标准 zstd 工具可免费导出纯文本 JSONL：

```powershell
zstd -d cloudbytes.tdict.zst -o cloudbytes.jsonl
```

首行中的 `dictionary.license` 保存来源许可声明；其余行可以用任意 JSONL 工具处理。导出的文本及再次分发的数据仍遵循原数据许可证，项目代码的 MIT 许可证不替代这些条款。

## 本次数据快照

| 文件 | 原 SQLite 包 | zstd 包 |
|---|---:|---:|
| `cloudbytes.tdict.zst` | 140.5 MiB | 15.7 MiB |
| `wordset.tdict.zst` | 50.5 MiB | 6.8 MiB |
| `simple-wiktionary.tdict.zst` | 26.8 MiB | 2.6 MiB |
| `wiktionary.tdict.zst` | 900.7 MiB | 137.0 MiB |
| 合计 | 1,118.5 MiB | 162.1 MiB |

四本完整数据均通过 `--verify`，压缩前后条目内容一致。压缩减少存储和传输体积，不减少全部加载后的常驻词典数据量。
