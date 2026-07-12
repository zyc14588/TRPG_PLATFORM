# Manifests README — v2.21 strict

当前 source manifest 由 `python scripts/ci/manifest.py --write` 从 Git 跟踪文件与本次新增文件确定性生成，并由 `python scripts/ci/verify_manifest.py` 校验。

`CURRENT_PACKAGE_MANIFEST.md`、`SELF_CONTAINED_PACKAGE_MANIFEST.md` 与根 `MANIFEST.md` 必须字节一致。三个生成输出不参与自身 Hash 集合，避免不可满足的自引用；其存在性与一致性由验证器强制检查。

归档级 ZIP size / SHA256 / entry count 不写入包内当前 manifest，原因是 ZIP hash 对包内容自引用。最终 ZIP 的真实 size、SHA256 和 `unzip -t` 结果由包外验收报告记录。

`V221_BUILD_SUMMARY.json.md`、`V221_STRICT_VALIDATION_REPORT.md` 与 `source-archive/superseded/**` 中的旧报告仅作 provenance，不得作为当前施工或当前验收依据。
