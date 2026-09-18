# 代码编译与部署规则

## 目标

构建 Linux 和 Windows 绿色部署包，并确保包内没有私有数据或缺失依赖。

## 标准构建

在仓库根目录执行：

- ./build.sh

脚本会安装前端依赖、构建前端、分别构建 Linux x64 和 Windows x64 后端，并收集文档与示例配置打包。

标准产物：

- bin/VisionHyperAgent-linux-x64.zip
- bin/VisionHyperAgent-windows-x64.zip

## 发布前检查

1. 确认 zip 中没有 app_config.json、logs/、真实密钥、测试证据或本机数据。
2. Linux 包内主程序必须保留可执行权限。
3. 检查可执行文件架构，并用 ldd 确认 Linux 动态依赖完整。
4. Linux 包应做解压、启动和 /api/health 冒烟验证，退出时确认进程清理干净。
5. Windows 包至少确认 zip 结构和 exe 架构；未实机验证时必须明确告知用户。
6. 输出包大小和 SHA256，便于用户核对远程拷贝结果。

## 运行约束

- 用户必须解压整个目录，不能只复制主程序。
- 首次运行需把 app_config.example.json 复制为 app_config.json 并填写私有模型服务信息。
- app_config.json 含密钥，只保存在用户本机，不进入 Git 和发布包。
- 浏览器关闭不退出后台；用户通过系统托盘菜单退出，后端负责清理 Codex 子进程。
- 日志位于应用目录 logs/ 下，文件名按天滚动。
