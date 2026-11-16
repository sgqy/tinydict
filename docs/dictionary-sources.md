# 词典收集清单

## 本轮选择

2026-11-17，优先选择可批量下载、可离线使用、有明确授权的纯英语数据。例句必须来自原来源，并保留义项关联与引文出处。不同词典独立存储和展示。

本轮接入两个 Wiktionary 版本。Simple English 的释义面向学习者，用较简单的英语解释常见词；英语 Wiktionary 补充更广的词汇、短语、用法标签和引用材料。两个版本拥有不同的词条内容，分别作为一本词典。来源介绍见 [Simple English Wiktionary](https://simple.wiktionary.org/wiki/Wiktionary:About) 和 [Kaikki 英语数据页](https://kaikki.org/dictionary/English/index.html)。

| 本机导入结果 | Simple English Wiktionary | English Wiktionary |
|---|---:|---:|
| 压缩源文件 | 4.5 MiB | 509.4 MiB |
| 保留条目 | 61,743 | 1,497,827 |
| 保留义项 | 73,204 | 1,793,712 |
| 例句及引文记录 | 25,492 | 770,517 |
| 含例句的义项 | 18,792（25.7%） | 385,285（21.5%） |
| 常驻数据估算 | 37.7 MiB | 1,193.7 MiB |

这些数字来自完整快照的实际导入。例句记录也包含短语和引文，不都是完整句子；跨来源可能重复，不能视为不同句子的总数。词头数包含屈折变化、专名和罕见词，不能据此判断学习价值。

## 质量与来源保留

Simple English 适合作为日常学习的补充；覆盖范围较小，部分释义和例句较短。英语 Wiktionary 的用法材料更丰富，但也包含古语、历史引文和社区编辑差异。程序保留原始标签，不把历史用法自动改写成现代英语。

导入保留层级释义、词性、用法标签、义项内同义词和原例句。引文的 `ref` 紧随正文显示，原词条链接可追溯到贡献者历史。词条级、未关联义项的同义词不复制到每个义项，避免制造错误关系。翻译、音频及其他暂不展示的字段仍保留在原始压缩文件中。

两本词典都只导入 `lang_code=en`。重定向单独计数，目前不作为别名索引；缺失语言字段或损坏 JSON 会报错。例句默认每个义项展示 3 条，按需继续展开，减少长词条的界面构建开销。

本地适配使用 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)，保存许可正文、署名、转换说明和输入哈希。数据提取来自 [Kaikki / Wiktextract](https://kaikki.org/dictionary/rawdata.html)。英语切片属于上游已弃用的后处理下载渠道，本轮锁定已下载的快照；后续迁移原始提取数据时需重新核对字段和数量。

## 后续候选

| 来源 | 可补充的内容 | 本轮处理 |
|---|---|---|
| [Tatoeba 英语语料](https://tatoeba.org/en/downloads) | 大量社区例句，提供英语单语导出；主要为 CC BY 2.0 FR，另有 CC0 子集 | 下一优先候选。它是语料库，应独立展示并建立句子检索，不将含某个单词的句子自动归到某个词义 |
| [CMUdict](https://github.com/cmusphinx/cmudict) | 美式英语读音、重音和读音变体 | 值得作为独立发音词典接入；当前释义模型需增加发音展示。授权见 [BSD 两条款许可](https://github.com/cmusphinx/cmudict/blob/master/LICENSE) |
| [Open English WordNet](https://en-word.net/downloads) | 语义关系、同义词组、释义和例句；CC BY 4.0 | CloudBytes 已含 OEWN 子来源，本轮不重复安装；若需要独立版本更新或语义关系浏览，再单独接入 |
| [GCIDE](https://gcide.gnu.org.ua/about) | Webster 1913 基础上的修订与补充，可离线下载 | 与现有 CloudBytes 的历史词典内容重叠，优先级较低；[GPL 3.0 或更新版本](https://gcide.gnu.org.ua/license) |

商业学习词典的开放网页或 API 不等于开放的离线数据授权，本轮未将其网页内容批量收集。合成词典暂不作为默认权威来源；若后续接入，应明确标识生成性质并单独评估错误率。
