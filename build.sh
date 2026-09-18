#!/usr/bin/env bash
# VisionHyperAgent reproducible build script.
# Outputs: bin/VisionHyperAgent, bin/VisionHyperAgent.exe, and the Windows zip.
set -euo pipefail
cd "$(dirname "$0")"

echo "=== 1/4 构建前端 ==="
cd source/frontend
npm ci --silent --no-audit --no-fund
npm run build

echo "=== 2/4 构建 Linux/当前平台 release ==="
cd ../backend
cargo build --release --bin vha-server

echo "=== 3/4 构建 Windows x64 release ==="
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  windows_exe="target/release/vha-server.exe"
else
  cargo build --target x86_64-pc-windows-gnu --release --bin vha-server
  windows_exe="target/x86_64-pc-windows-gnu/release/vha-server.exe"
fi

echo "=== 4/4 收集产物并打包 ==="
cd ../..
mkdir -p bin bin/test-artifacts/final
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  cp "$windows_exe" bin/VisionHyperAgent.exe.next
  mv bin/VisionHyperAgent.exe.next bin/VisionHyperAgent.exe
else
  cp source/backend/target/release/vha-server bin/VisionHyperAgent.next
  chmod +x bin/VisionHyperAgent.next
  mv bin/VisionHyperAgent.next bin/VisionHyperAgent
  cp "source/backend/$windows_exe" bin/VisionHyperAgent.exe.next
  mv bin/VisionHyperAgent.exe.next bin/VisionHyperAgent.exe
fi

python3 - <<'PY'
from pathlib import Path
import hashlib
import json
import zipfile

root = Path.cwd()
bin_dir = root / "bin"
readme = """VisionHyperAgent Windows 部署说明
================================

1. 解压整个目录，不要只复制 exe。
2. 将 app-config.example.json 复制为 exe 同目录下的 app_config.json。
3. 编辑 app_config.json：
   - providers[].baseUrl 填上游 /v1 地址；
   - providers[].apiKey 填私有密钥；
   - models[].modelId 是前端显示和请求使用的唯一 ID；
   - models[].modelName 填上游真实模型名；
   - agent.activeModelId 必须指向已配置的 modelId。
4. Windows 需要同平台原生 Codex 可执行文件。codex.executable 为空时，程序会按应用目录、depends/codex 和 PATH 查找。
5. 本版本使用 app_config.json，不再使用 config.local.toml。
6. app_config.json 包含密钥，不要提交到 Git、不要随压缩包分发。
7. 双击 VisionHyperAgent.exe 启动；浏览器关闭不会退出后台。需要退出时使用系统托盘图标的“退出”菜单。
8. 日志位于 logs/VisionHyperAgent.YYYY-MM-DD.log。

更多说明见 docs/codex-agent-usage.md。
"""
(bin_dir / "README-Windows.txt").write_text(readme, encoding="utf-8")

zip_path = bin_dir / "VisionHyperAgent-windows-x64.zip"
temporary = zip_path.with_suffix(".zip.next")
entries = [
    bin_dir / "VisionHyperAgent.exe",
    root / "docs/app-config.example.json",
    root / "docs/codex-agent-usage.md",
    root / "docs/codex-agent-test-report.md",
    root / "docs/frontend-backend-architecture.md",
    bin_dir / "README-Windows.txt",
]
with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for path in entries:
        archive.write(path, path.name)
temporary.replace(zip_path)

def artifact(path: Path):
    content = path.read_bytes()
    return {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}

artifacts = {
    path.name: artifact(path)
    for path in [
        bin_dir / "VisionHyperAgent",
        bin_dir / "VisionHyperAgent.exe",
        zip_path,
    ]
    if path.exists()
}
(bin_dir / "test-artifacts/final/artifacts.json").write_text(
    json.dumps(artifacts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
PY

echo "✅ 完成:"
ls -lh bin/VisionHyperAgent bin/VisionHyperAgent.exe bin/VisionHyperAgent-windows-x64.zip
