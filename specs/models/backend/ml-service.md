# ml-service API 文档

## Overview

本文件同时是 `ml-service` 的 API 契约和 2119 需求来源。需求约束训练、评估、推理任务的参数、资源和结果登记。

## Requirements

### 1: 模型任务治理

1. ML 服务 MUST 仅为有效 manifest、模型和已确认参数创建任务 [manual]
2. ML 服务 MUST 在 GPU 或显存不足时返回明确资源状态，不得伪造任务可运行 [manual]
3. ML 服务 MUST 将成功、失败和取消结果关联到任务、模型、参数和产物 [manual]
4. ML 服务 MUST 拒绝使用不同参数复用同一任务 ID [manual]


## 1. 概述

`ml-service` 负责训练、评估、推理任务的业务校验、模型候选登记和资源策略；它不执行 GPU 计算，而是把合法任务交给 `task-runtime`。

## 2. 模块定义

| 项目 | 内容 |
| --- | --- |
| 架构层 | backend |
| 语言 | Rust |
| 公开边界 | ML 任务请求、模型候选和指标快照 |
| 不负责 | 直接启动 Python、计算指标、修改 manifest |
| 资源策略 | 同时最多一个 GPU 训练任务；评估和推理按运行时排队 |

## 3. 数据结构

| 类型 | 字段 | 说明 |
| --- | --- | --- |
| `TrainRequest` | dataset_id、parameters、plan_id | 已确认训练计划 |
| `EvalRequest` | model_id、dataset_id | 评估输入 |
| `InferenceRequest` | model_id、image_ids | 新图推理输入 |
| `TrainParameters` | epochs、batch_size、learning_rate、image_size、seed | 训练参数 |
| `ModelCandidate` | id、weight_path、dataset_id、metrics、status | 候选模型 |
| `MetricsSummary` | map、iou、per_category、failure_count | 指标摘要 |
| `ResourceReport` | cuda、device、vram_bytes、available | GPU 环境报告 |
| `MlTaskReceipt` | task_id、kind、accepted | 任务受理结果 |

## 4. 属性

| 属性 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `max_batch_size` | `u32` | 配置值 | 业务允许的上限 |
| `active_model_id` | `Option<ModelId>` | `None` | 当前候选模型 |
| `resource_report` | `ResourceReport` | 未检测 | 最近一次环境报告 |
| `training_policy` | `TrainingPolicy` | 串行 | GPU 任务并发策略 |

## 5. 方法

### `probe_resources() -> ResourceReport`

通过 task-runtime 请求 Worker 检测 Python、PyTorch、CUDA、设备和显存，并保存报告。

### `suggest_parameters(dataset_id) -> ParameterContext`

返回给 agent-service 的数据统计和参数边界；不自动启动训练。

### `start_training(request) -> MlTaskReceipt`

校验数据集、参数、GPU 报告和已确认计划后创建任务。

### `start_evaluation(request) -> MlTaskReceipt`

校验模型处于可评估状态并创建评估任务。

### `start_inference(request) -> MlTaskReceipt`

校验测试图片和模型后创建推理任务。

### `register_result(result) -> ModelCandidate`

仅接受 task-runtime 验证过完成凭证的结果；半成品不能进入候选列表。

### `list_candidates(project_id) -> Vec<ModelCandidate>`

返回模型候选和指标摘要，不读取权重文件。

### `select_candidate(model_id) -> ModelCandidate`

把已存在候选设置为当前模型，写入选择事件。

## 6. 消息与事件

任务创建后由 task-runtime 发布 `ml.task.*` 事件；ml-service 只发布 `model.candidate.created`、`model.selected` 和 `ml.resource.updated`。指标事件只携带摘要和任务 ID。

## 7. 委托与回调

`TaskSubmitter` 委托用于向运行时提交结构化任务；`ResultValidator` 回调负责检查完成凭证、清单和哈希。回调必须幂等，同一 task_id 重复登记返回已登记结果。

## 8. 错误处理

| 错误码 | 说明 |
| --- | --- |
| `dataset_not_ready` | 数据集不存在或无确认标注 |
| `parameters_invalid` | 参数超出边界 |
| `gpu_unavailable` | 当前环境不满足训练/推理要求 |
| `model_not_ready` | 模型缺少有效权重和凭证 |
| `task_conflict` | 资源策略不允许当前任务 |
| `result_untrusted` | 完成凭证、清单或哈希不匹配 |
| `candidate_not_found` | 模型候选不存在 |

## 9. 生命周期与线程安全

服务启动时加载模型候选和最近资源报告；方法可并发调用，任务创建使用数据库幂等键。模型候选状态由服务串行更新，读取返回快照。关闭时停止受理新任务，等待运行时返回最终状态。

## 10. 依赖关系

依赖 `project-service` 读取数据集和图片、`task-runtime` 创建任务、`storage` 登记结果、`common-services` 做参数和路径校验。被 `api-gateway`、`workspace-ui` 和 `agent-service` 使用。

## 11. 调用示例

```rust
// 创建 ML 服务并探测环境
let ml = MlService::new(projects, runtime, storage, policy);
let resources = ml.probe_resources().await?;
// 读取 Agent 需要的数据集摘要
let context = ml.suggest_parameters(dataset_id).await?;
// 用户确认计划后提交训练
let receipt = ml.start_training(TrainRequest {
    dataset_id,
    parameters: TrainParameters { epochs: 20, batch_size: 2, learning_rate: 0.0001, image_size: 1024, seed: 7 },
    // 固定用户确认的训练计划
    plan_id: confirmed_plan,
}).await?;
// 训练完成后登记可信结果
let candidate = ml.register_result(ValidatedTrainingResult::from_task(receipt.task_id)).await?;
// 查询候选模型后执行评估、推理和选择
let candidates = ml.list_candidates(project_id).await?;
ml.start_evaluation(EvalRequest { model_id: candidate.id, dataset_id }).await?;
ml.start_inference(InferenceRequest { model_id: candidate.id, image_ids: test_images }).await?;
ml.select_candidate(candidate.id).await?;
// 验证资源警告和候选模型结果
assert!(resources.available || context.gpu_warning.is_some());
// 确认登记的候选模型可查询
assert!(candidates.iter().any(|item| item.id == candidate.id));
```

| 方法覆盖 | Demo 位置 |
| --- | --- |
| `new` / `probe_resources` / `suggest_parameters` | 初始化和环境段 |
| `start_training` / `register_result` | 训练登记段 |
| `start_evaluation` / `start_inference` / `select_candidate` | 评估、推理和选择段 |
| `list_candidates` | 候选查询代码 |

## 12. 2119 测试要求

| 2119 ID | 验证行为 | 当前证据 |
| --- | --- | --- |
| `ml-service.1.1` | 非法输入和未确认参数不能入队 | 规划验收；实现后补任务校验测试 |
| `ml-service.1.2` | 资源不足状态明确且不伪造成功 | 规划验收；实现后补 GPU 能力测试 |
| `ml-service.1.3` | 结果与任务和产物可追溯 | 规划验收；实现后补登记集成测试 |
| `ml-service.1.4` | 参数冲突的任务 ID 被拒绝 | 规划验收；实现后补幂等测试 |

测试使用 `// 2119: ml-service.1.1` 标记，并覆盖与 `task-runtime` 的真实协议。
