# Manifests README — v2.21 strict

当前目录只保存 v2.21 包内 Markdown 文件 manifest、构建摘要和严格校验报告。

归档级 ZIP size / SHA256 / entry count 不写入包内当前 manifest，原因是 ZIP hash 对包内容自引用。最终 ZIP 的真实 size、SHA256 和 `unzip -t` 结果由包外验收报告记录。

`source-archive/superseded/**` 中的旧 manifest、旧 ZIP 校验报告和旧验收报告仅作 provenance，不得作为当前施工或当前验收依据。
