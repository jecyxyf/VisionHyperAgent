#!/usr/bin/env bash
# VisionHyperAgent 一键构建脚本
# 产出: bin/VisionHyperAgent[.exe]
set -euo pipefail
cd "$(dirname "$0")"

echo "=== 1/3 构建前端 ==="
cd source/frontend
npm install --silent
npm run build

echo "=== 2/3 构建后端（release）==="
cd ../backend
cargo build --release --bin vha-server

echo "=== 3/3 复制产物 ==="
cd ../..
mkdir -p bin
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  cp source/backend/target/release/vha-server.exe bin/VisionHyperAgent.exe
  echo "✅ 完成: bin/VisionHyperAgent.exe"
else
  cp source/backend/target/release/vha-server bin/VisionHyperAgent
  chmod +x bin/VisionHyperAgent
  echo "✅ 完成: bin/VisionHyperAgent"
fi
