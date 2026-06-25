# ADR-0003：MVP 地图几何

- 状态：Accepted
- 决策：支持 scene board、square、hex flat-top、hex pointy-top。
- 后果：位置用世界坐标；距离、邻接、范围和吸附通过 GridGeometry 适配器。等距与 3D 延后。
