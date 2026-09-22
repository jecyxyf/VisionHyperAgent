# mask-rcnn-worker API 文档

## 1. 概述

`mask-rcnn-worker` 是唯一执行 PyTorch / torchvision Mask R-CNN 计算的 Python 子进程。它通过 stdin / stdout NDJSON 与 Rust `task-runtime` 通信，读取明确授权的 manifest，写入任务目录，不连接 SQLite。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | ml-worker |
| 语言 | Python 3.11 |
| 公开边界 | NDJSON 命令、事件、结果凭证和任务文件 |
| 运行方式 | Rust 按需启动，不常驻；单任务单进程 |
| 安全边界 | 仅访问命令指定目录，不执行 shell 字符串，不读取无关环境变量 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `WorkerCommand` | request_id、kind、payload | NDJSON 输入命令 |
| `EnvironmentReport` | python、torch、cuda、device、vram | 环境检测结果 |
| `TrainConfig` | manifest、epochs、batch_size、lr、image_size、seed | 训练配置 |
| `WorkerEvent` | request_id、kind、progress、metrics、message | stdout 事件 |
| `InferenceResult` | image_id、instances、visualization_path | 推理结果 |
| `CompletionProof` | status、artifacts、hashes、finished_at | 完成凭证 |
| `WorkerError` | code、message、trace_id | 脱敏错误 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `device` | `torch.device` | 自动选择 | CUDA 可用时使用 GPU |
| `cancel_requested` | `bool` | `False` | 协作式取消标志 |
| `task_root` | `Path` | 命令指定 | 任务目录 |
| `random_seed` | `int` | 配置值 | 可复现实验种子 |
| `protocol_version` | `int` | 1 | NDJSON 协议版本 |

## 5. 方法

### `probe_environment() -> EnvironmentReport`

检测 Python、PyTorch、torchvision、CUDA 设备和显存；不修改驱动和系统环境。

### `load_manifest(path) -> DatasetManifest`

读取并校验 manifest、图片路径、遮罩引用和哈希；拒绝越出任务根目录的路径。

### `train(config) -> CompletionProof`

加载 Mask R-CNN、执行训练、保存 checkpoint、指标曲线、最终权重和完成凭证。

### `evaluate(model_path, manifest, output_dir) -> CompletionProof`

计算 mask mAP、IoU、类别指标和失败案例，写入评估凭证。

### `infer(model_path, image_paths, output_dir) -> CompletionProof`

对新图片生成实例遮罩、类别、置信度和可视化结果。

### `request_cancel() -> None`

设置取消标志；在 epoch 或图片边界停止后写出取消结果。

### `run_ndjson(stdin, stdout) -> int`

处理命令流并输出事件；每条输入只对应一个终态凭证。

## 6. 消息与事件

输入命令为 `probe`、`train`、`evaluate`、`infer`、`cancel`、`shutdown`。输出事件为 `started`、`progress`、`metric`、`log`、`cancelled`、`completed`、`failed`。事件载荷不包含 API Key、完整 traceback 或原图像素。

## 7. 委托与回调

`EventWriter` 将事件序列化为一行 JSON；`MetricCallback` 接收 epoch 指标；`CancelChecker` 在安全检查点读取取消状态。回调异常必须转为 `worker_callback_error`，不能破坏 NDJSON 帧边界。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `environment_unavailable` | Python、PyTorch 或 CUDA 不可用 |
| `manifest_invalid` | manifest、哈希或路径错误 |
| `model_load_failed` | 权重和配置无法加载 |
| `out_of_memory` | 显存不足 |
| `cancelled` | 用户请求取消 |
| `artifact_write_failed` | 产物写入失败 |
| `protocol_error` | NDJSON 命令非法 |
| `worker_internal_error` | 未分类 Worker 故障 |

任何训练或推理完成都必须同时写出 `completion.json`、产物清单和哈希；缺失时输出失败或中断。

## 9. 生命周期与线程安全

进程启动后先处理 probe，再接受一个任务命令；单个 Worker 不并发执行多个训练任务。DataLoader 和 GPU 使用在 Python 进程内协调，stdout 写入由单一事件写出器串行化。收到 cancel 后不强杀当前安全检查点；退出前 flush 结果和日志。

## 10. 依赖关系

依赖 Python、PyTorch、torchvision、Pillow、NumPy 和任务目录；被 `task-runtime` 以受控子进程启动。不得依赖 SQLite、HTTP、Svelte 或 Codex。

## 11. 调用示例

```python
# 启动 Worker 并检测 CUDA 环境
worker = MaskRcnnWorker(task_root=Path("data/tasks/task-001"))
environment = worker.probe_environment()
# 读取 Rust 生成的 manifest
manifest = worker.load_manifest(Path("data/tasks/task-001/manifest.json"))
# 执行训练并在事件中观察指标
proof = worker.train(TrainConfig(
    manifest=manifest,
    epochs=20,
    # 设置训练批次
    batch_size=2,
    learning_rate=1e-4,
    # 设置输入尺寸
    image_size=1024,
    seed=7,
))
# 评估、推理和取消接口均保持同一任务目录边界
worker.evaluate(proof.model_path, manifest, Path("data/tasks/task-001/eval"))
worker.infer(proof.model_path, [Path("test/a.png")], Path("data/tasks/task-001/infer"))
# 请求在安全检查点取消任务
worker.request_cancel()
# 通过 NDJSON 入口处理协议命令
exit_code = worker.run_ndjson(stdin, stdout)
# 校验协议退出码和训练产物
assert exit_code in (0, 1)
assert proof.artifacts and environment.device is not None
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `probe_environment` / `load_manifest` | 初始化和 manifest 段 |
| `train` / `evaluate` / `infer` | 计算段 |
| `request_cancel` | 取消段 |
| `run_ndjson` | NDJSON 入口调用代码 |
