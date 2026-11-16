# TinyDict 数据集预发布 · 2026-11-17

本次上传的附件仅包含四个独立词典数据包，总计 162.1 MiB。每本词典独立加载和查询。程序、安装包、校验文件和许可文件均不作为额外附件上传；许可声明已嵌入各数据包，校验值列在下方。

## 数据包

| 附件 | 条目 | 义项 | 文件大小 |
|---|---:|---:|---:|
| `cloudbytes.tdict.zst` | 388,587 | 388,587 | 16,441,130 字节 |
| `wordset.tdict.zst` | 108,128 | 163,274 | 7,147,971 字节 |
| `simple-wiktionary.tdict.zst` | 61,743 | 73,204 | 2,704,882 字节 |
| `wiktionary.tdict.zst` | 1,497,827 | 1,793,712 | 143,645,639 字节 |

这是 TinyDict 导入后的英语词典快照，包含程序支持的释义、例句、标签和来源信息。原始提取数据中的翻译、音频等未使用字段不包含在包内。Simple English 和英语 Wiktionary 分别保留 25,492 条和 770,517 条例句及引文记录，跨来源未去重。

## 使用

需要支持 `.tdict.zst` 的 TinyDict 代码版本，可使用本发布标签对应的源码构建。将数据包放入程序旁的 `data/dictionaries`，或通过 `--data-dir` 指定目录。程序在启动时流式解压到内存；日常查询不访问磁盘。

在源码项目中，将四个附件放入 `data/dictionaries` 后运行：

```powershell
./scripts/setup.ps1 -SkipDownload
./dist/tinydict.exe
```

无需另行下载原始词典。四本词典全部加载后的常驻数据估算约为 1.47 GiB，另有进程及界面开销。4 GB 档位会拒绝加载完整英语 Wiktionary，其他三本仍按可用内存判断。

文件采用标准 zstd 压缩的 UTF-8 JSONL，格式版本为 2。首行是清单，含 `dictionary.license`、来源版本和输入哈希；后续各行是词条。也可使用标准 zstd 工具免费导出纯文本，无需 TinyDict：

```powershell
zstd -d cloudbytes.tdict.zst -o cloudbytes.jsonl
```

## 数据许可证与署名

**这些数据包不使用项目代码的 MIT 许可证。** 各来源及本次数据适配遵循下表所列条款，完整上游声明保留在包内。项目代码仍为 MIT。

| 数据集 | 来源与署名 | 数据许可 |
|---|---|---|
| CloudBytes | CloudBytes-Academy 整理；Webster 1913 经 OPTED、Project Gutenberg 和 SourceForge MySQL English Dictionary；Open English WordNet Team 及 Princeton University | `webster1913` 部分在美国属于公有领域，并保留 OPTED 使用条件；`oewn` 部分为 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)，同时保留 WordNet 3.1 声明。见固定版本的 [LICENSE-DATA.md](https://github.com/CloudBytes-Academy/English-Dictionary-Open-Source/blob/ef5562840cf57c7682fc63e7e35ddb616638f93b/v2/LICENSE-DATA.md) |
| Wordset | Wordset, Inc.、原贡献者和编辑者；Princeton University WordNet 3.0 | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)，同时保留 WordNet 3.0 许可声明。见固定版本的 [LICENSE](https://github.com/wordset/wordset-dictionary/blob/4503bfafbe4556361a296b9d368c18168dfb56c3/LICENSE) |
| Simple English Wiktionary | [Simple English Wiktionary 贡献者](https://simple.wiktionary.org/)，由 [Kaikki / Wiktextract](https://kaikki.org/dictionary/rawdata.html) 提取 | 本次适配以 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) 分发；许可正文和署名保留在包内 |
| English Wiktionary | [English Wiktionary 贡献者](https://en.wiktionary.org/)，由 [Kaikki / Wiktextract](https://kaikki.org/dictionary/English/index.html) 提取 | 本次适配以 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) 分发；许可正文和署名保留在包内 |

CloudBytes 包内完整保留上游的 OPTED、OEWN 和 WordNet 声明。OPTED 条件包括内容继续免费、提供可访问的纯文本和注明来源；上游还提示商用前联系 Project Gutenberg。本格式可按上面的命令导出纯文本，Webster 内容不因压缩而改变其公有领域属性。

OEWN 署名：Copyright 2019–present, The Open English WordNet Team；其源自 Princeton WordNet，相关声明包括 WordNet 3.1 Copyright 2011 by Princeton University。Wordset 的源数据声明另含 WordNet 3.0 Copyright 2006 by Princeton University。再次分发时需保留这些原声明。

Wordset 词条保留原贡献者和编辑者字段。两版 Wiktionary 的原词条及贡献者历史，可通过对应站点的 `/wiki/<URL 编码词头>` 和页面历史查看；TinyDict 也提供原词条入口。引文保留上游出处，引用材料可能另有适用条款。

TinyDict 的转换包括筛选有释义的英语条目、整理程序支持的字段、过滤空例句、建立查询索引，以及使用 zstd 压缩存储。Wiktionary 的层级释义和引文出处按顺序保留；词典之间不合并。包内还保留上游自身的转换说明。本次压缩转换已逐条验证，与此前 SQLite 包的词条、义项、例句、署名和导入报告一致。Wordset 和两版 Wiktionary 的数据适配继续采用 CC BY-SA 4.0。

## 固定来源版本

- CloudBytes：提交 `ef5562840cf57c7682fc63e7e35ddb616638f93b`。
- Wordset：提交 `4503bfafbe4556361a296b9d368c18168dfb56c3`。
- Simple English Wiktionary：Kaikki 文件时间 2026-10-30 20:48:42 UTC；压缩源文件 SHA-256 为 `79afd871e342db3b87c3f2044ed21efa312e4c4ee006814b57c4cf4536ef70a1`。
- English Wiktionary：Kaikki 文件时间 2026-10-30 23:09:26 UTC；压缩源文件 SHA-256 为 `4d03b6cba283744756c57ff661c2c6cbfe34fcf8ff513a966576cf7d97e91a99`。该英语切片来自上游已弃用的后处理下载渠道，本发布固定保存当前快照。

## 附件 SHA-256

```text
54a19f4525e5bfbcc71cbd1bb1c06565c53eea4f6d0bde117f07bc5c46d9d6cd  cloudbytes.tdict.zst
ed330327b43d614660af089de469a7ceb14a49f298f18aa123d220a8bbbb0873  wordset.tdict.zst
7cc3b46e5a9fa67570e1f2e1918e7d3ffb82648ca7ffd96977b8ea1f92fa57d1  simple-wiktionary.tdict.zst
4d4c50da8aa9ad6ffc1ed24c63125bdb76c3a21b3b60ec5a68e46184ff33611e  wiktionary.tdict.zst
```

验证时可用 PowerShell 的 `Get-FileHash -Algorithm SHA256 <文件>`。四个附件均已完成全量加载和逐条内容校验；配套代码通过 Clippy、18 项测试及文档构建。

GitHub 会自动显示标签对应的 Source code（zip / tar.gz）链接，这是平台自动生成的归档入口。本次手工上传的发布附件只有上表中的四个数据包，未上传程序或其他文件。参见 [GitHub Releases 说明](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)。
