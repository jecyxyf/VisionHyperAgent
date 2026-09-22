# <模块名 / 类名> API 文档

> 本模板使用同目录《技术方案模板.md》中的 VmBatchRunner 示范模块 API 的表达形式，不代表目标项目已有实现。依据已确认的技术方案替换模块、类型、语言和接口，输出到 `specs/models/<架构层名称>/<模块名>.md`；没有的能力说明“不适用”，不为填表添加功能。全部方法的调用集中在末尾一个完整 Demo 中，成品移除本段模板说明。

## 1. 概述

VmBatchRunner 属于 ImageBatch 的 ViewModel 层，负责提交图片批处理任务、调用 Algo 中的编解码与滤镜、协调取消，并向调用方报告进度和完成结果。

VmTaskManager 可以将 Object 中的任务转为提交快照，再根据结果更新业务对象。任务调度不进入 UI，滤镜算法不进入 ViewModel，也不新增架构层。

本 Demo 同一时间只运行一个任务，使用 C++17 和 Qt Core，不要求创建图形界面。

---

## 2. 模块定义

### 2.1 继承关系

```
QObject
└── VmBatchRunner
```

### 2.2 命名空间 / 所属模块

- **命名空间**：`ImageBatch::ViewModel`。
- **所属库**：`ViewModel`。
- **示例头文件**：`ViewModel/VmBatchRunner.h`。
- **对象约束**：不可复制或移动；公开方法在对象所属线程调用。
- **对外边界**：公开任务数据、方法和通知契约，不公开工作线程、锁、第三方图像类型或私有成员。

### 2.3 驱动对象与组件归属

| 项目 | 内容 |
|------|------|
| 所属驱动对象模块 | `<model 模块名>`；若当前文档本身就是 model，填写自身 |
| 组件名称 | `<component 名称>`；model 文档填写“无” |
| 上层调用入口 | `<上层只调用 model，或所属 model 的公开方法>` |
| 依赖关系 | `<组件依赖所属 model 的哪些契约；model 依赖哪些组件>` |
| 源码位置 | `<model>/` 或 `<model>/components/<component>/` |
| 文档位置 | `specs/models/<架构层>/<model>.md` 或 `specs/models/<架构层>/<model>/components/<component>.md` |

复杂对象使用以下源码归属关系：

```text
modelA/
├── modelA 源代码
└── components/
    └── componentX 源代码
```

一个 model 可以包含多个 component。组件文档必须在一个对象文件中完整描述该组件的所有公开数据结构、属性、方法、消息、委托和调用示例；组件内部辅助对象不再生成拆分文档，也不能绕过所属 model 成为上层直接入口。

---

## 3. 数据结构

除 Algo::FilterParams 外，本节类型均位于 ImageBatch::ViewModel。结构体按值持有数据，不包含控件或业务对象的裸指针。

### 3.1 枚举

**TaskState — 运行状态**

| 取值 | 说明 |
|------|------|
| `Idle` | 尚未提交任务 |
| `Running` | 任务正在执行；取消请求尚未生效时仍保持该状态 |
| `Succeeded` | 全部图片处理并保存成功 |
| `Failed` | 任务失败，已成功输出的结果保留 |
| `Cancelled` | 任务在安全检查点接受取消并停止 |

**EventKind — 通知类型**

| 取值 | 说明 |
|------|------|
| `Started` | 任务已被接受并开始执行 |
| `Progress` | 一张图片完成处理与保存 |
| `Finished` | 任务结束，具体结果由 TaskState 区分 |

**ErrorCode — 错误类型**：取值及处理方式统一见“错误处理”章节。

### 3.2 类型别名

| 类型名 | 定义 | 约束 |
|------|------|------|
| `SubscriptionToken` | `std::uint64_t` | 0 表示无效；同一实例内不重复使用，且只在创建它的实例中有效 |
| `Algo::FilterParams` | `std::map<std::string, std::variant<int, double, bool, std::string>>` | 由 Algo 定义，键名和取值约束由具体滤镜决定 |
| `TaskEventHandler` | `std::function<void(const TaskEvent&)>` | 事件接收委托；完整调用契约见“委托与回调”章节 |

### 3.3 InputImage — 输入图片

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `imageId` | `std::string` | 空 | 必填，在同一请求内唯一；仅允许匹配 `[A-Za-z0-9_-]+` 的标识 |
| `path` | `std::string` | 空 | 必填，UTF-8 编码的输入文件绝对路径 |

### 3.4 OperationSpec — 处理操作

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `filterId` | `std::string` | 空 | 必填，例如 resize、grayscale 或插件提供的处理标识 |
| `params` | `Algo::FilterParams` | 空集合 | 操作参数；例如 resize 接受正整数 width 和 height |

### 3.5 BatchRequest — 提交请求

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `taskId` | `std::string` | 空 | 必填，用于关联请求、通知和输出文件；仅允许匹配 `[A-Za-z0-9_-]+` 的标识 |
| `inputs` | `std::vector<InputImage>` | 空集合 | 至少一张图片；imageId 不得重复，字段约束见 InputImage |
| `operations` | `std::vector<OperationSpec>` | 空集合 | 至少一个处理操作，严格按数组顺序执行 |
| `outputDirectory` | `std::string` | 空 | 必填，UTF-8 编码的输出目录绝对路径，不得指向本次输入文件 |

请求由 start() 校验并复制；成功返回后，调用方可以修改或销毁原请求。原始图片文件在任务结束前应保持可读且不被替换。

### 3.6 ApiError — 错误信息

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `code` | `ErrorCode` | `None` | 机器可判断的错误类型 |
| `message` | `std::string` | 空 | 用于定位原因的文字，不用于业务逻辑匹配 |

### 3.7 StartResult — 提交结果

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `accepted` | `bool` | `false` | 仅表示任务是否被接受，不表示处理成功 |
| `error` | `ApiError` | 无错误 | accepted=false 时提供拒绝原因；accepted=true 时为 None |

### 3.8 TaskSnapshot — 状态快照

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `taskId` | `std::string` | 空 | 当前或最近一次被接受的任务标识 |
| `state` | `TaskState` | `Idle` | 当前运行状态 |
| `completed` | `std::size_t` | 0 | 已完成处理且成功保存的图片数 |
| `total` | `std::size_t` | 0 | 本次任务输入图片总数 |
| `cancelRequested` | `bool` | `false` | 是否提交过取消请求，不等同于最终状态为 Cancelled |
| `outputPaths` | `std::vector<std::string>` | 空集合 | 已成功保存的输出路径，数量与 completed 一致 |
| `error` | `ApiError` | 无错误 | Failed 状态提供原因；Succeeded、Cancelled 的错误类型为 None |

快照按值返回。修改快照不会改变模块状态，快照也不会随任务自动更新。

### 3.9 TaskEvent — 消息载荷

| 字段 | 类型 | 默认值 | 说明 |
|------|------|------|------|
| `kind` | `EventKind` | 发送方必填 | Started、Progress 或 Finished |
| `snapshot` | `TaskSnapshot` | 发送方必填 | 事件产生时的快照副本，包含任务标识、状态、进度和错误 |

---

## 4. 属性

本 Demo 没有公开可写属性。以下状态通过 snapshot() 读取，不暴露可被外部修改的成员。

| 属性名 | 类型 | 访问方式 | 默认值 | 说明 |
|------|------|------|------|------|
| `state` | `TaskState` | 只读：snapshot().state | `Idle` | 运行状态 |
| `taskId` | `std::string` | 只读：snapshot().taskId | 空 | 用于关联任务和通知 |
| 进度 | completed / total | 只读：snapshot() | 0 / 0 | total=0 时尚无任务，调用方不能直接做除法 |
| `cancelRequested` | `bool` | 只读：snapshot().cancelRequested | `false` | 显示取消请求是否已提交 |

---

## 5. 方法

本节只定义签名、参数、返回值及行为；全部调用集中在最后的完整 Demo。

### 5.1 VmBatchRunner(registry, codec, parent)

```cpp
explicit VmBatchRunner(Algo::FilterRegistry& registry, Algo::ImageCodec& codec, QObject* parent = nullptr);
```

**说明**：创建任务执行器，初始状态为 Idle，不自动启动任务。

**参数**：

| 参数名 | 类型 | 说明 |
|------|------|------|
| `registry` | `Algo::FilterRegistry&` | 已初始化的滤镜注册表；借用，不转移所有权 |
| `codec` | `Algo::ImageCodec&` | 图片编解码服务；借用，不转移所有权 |
| `parent` | `QObject*` | 可选 Qt 父对象；必须与执行器属于同一线程 |

**生命周期**：registry 和 codec 必须晚于执行器销毁；任务活动期间不得修改注册表或卸载正在使用的插件。

### 5.2 ~VmBatchRunner()

```cpp
~VmBatchRunner() override;
```

**说明**：停止分发通知，请求取消任务，等待内部工作线程结束并释放资源。

**约束**：析构可能等待当前不可中断的图像操作完成，不保证立即返回；必须在所属线程执行，不能在事件回调内部直接销毁执行器。

### 5.3 subscribe(handler)

```cpp
SubscriptionToken subscribe(TaskEventHandler handler);
```

**说明**：注册事件接收委托，可以注册多个订阅。

**参数**：

| 参数名 | 类型 | 说明 |
|------|------|------|
| `handler` | `TaskEventHandler` | 必填；执行器持有一份委托，捕获对象的有效期由调用方保证 |

**返回值**：`SubscriptionToken` — 非零订阅令牌。新订阅只接收后续分发的事件，不补发历史记录；当前状态可通过 snapshot() 获取。

**异常**：空委托抛出 `std::invalid_argument`；分配失败按 C++ 异常机制处理。

### 5.4 unsubscribe(token)

```cpp
bool unsubscribe(SubscriptionToken token);
```

**说明**：取消指定订阅，释放该订阅持有的委托。

**参数**：

| 参数名 | 类型 | 说明 |
|------|------|------|
| `token` | `SubscriptionToken` | 必须使用同一执行器返回的令牌 |

**返回值**：`bool` — 实际移除订阅返回 true；无效或已取消的令牌返回 false。

**行为约束**：可以在回调内取消订阅；当前回调执行到返回，此后不再调用该委托，包括尚未分发的排队通知。

### 5.5 start(request)

```cpp
StartResult start(const BatchRequest& request);
```

**说明**：校验并复制请求，异步执行批处理。

**参数**：

| 参数名 | 类型 | 说明 |
|------|------|------|
| `request` | `const BatchRequest&` | 任务请求；字段约束见 BatchRequest，操作参数还须满足对应滤镜要求 |

**返回值**：`StartResult` — accepted=true 表示进入 Running，实际结果通过 Finished 通知；accepted=false 时返回错误，不改变现有任务快照，也不为被拒绝的请求发送事件。

**调用约束**：

- Idle 或任务终态下允许提交下一任务；Running 时拒绝并返回 Busy。
- 接受新任务时重新初始化快照：taskId 与 total 来自请求，completed 为 0、cancelRequested 为 false、outputPaths 为空、error 为 None。
- 同步检查请求结构、操作标识及参数，不在调用线程处理图片；输入文件读取失败通过异步结果报告。
- 接受后按图片顺序、操作顺序执行，输出命名为 `<taskId>_<imageId>.png`；不得覆盖输入文件或已有输出，冲突通过 WriteFailed 报告。
- 该任务的所有回调均在 start() 返回后经事件循环分发，不在 start() 调用栈内触发。
- 不在回调中重入 start()；连续任务应等当前通知返回后再提交。

### 5.6 cancel()

```cpp
bool cancel();
```

**说明**：对当前任务提交协作式取消请求，不强杀线程。

**参数**：无。

**返回值**：`bool` — Running 且首次登记取消请求时返回 true；无任务、任务已结束或已经请求取消时返回 false。

**行为约束**：请求成功后 cancelRequested=true，状态暂时仍为 Running；后台在图片或操作之间检查取消。若结果已经完成或发生错误，最终状态仍可能为 Succeeded 或 Failed，不能把 true 当成已经取消。

### 5.7 snapshot()

```cpp
TaskSnapshot snapshot() const;
```

**说明**：读取当前状态副本，不等待任务结束、不触发事件。

**参数**：无。

**返回值**：`TaskSnapshot` — 初始为空闲快照；任务结束后保留最终结果，直到下一任务被接受。返回值的使用不受执行器后续变化影响。

---

## 6. 消息与事件

这里的“消息”指模块内的类型化通知，不是 HTTP 或 WebSocket 报文。所有消息使用 TaskEvent，由 VmBatchRunner 经已注册的 TaskEventHandler 发送给调用方。

| 消息 | 触发条件 | 载荷约束 | 发送次数 |
|------|------|------|------|
| `Started` | 任务被接受并开始 | snapshot.taskId 对应请求，state=Running，completed=0 | 每个被接受的任务一次 |
| `Progress` | 一张图片完成全部操作并成功写出 | completed 递增，outputPaths 增加本次结果 | 零次或多次 |
| `Finished` | 任务成功、失败或接受取消 | state 为 Succeeded / Failed / Cancelled，携带最终快照 | 正常存活期间每个被接受的任务一次 |

**顺序约束**：Started 在前，Finished 在后；Finished 后不再分发该任务的进度。同一任务的消息按顺序分发，不能仅按到达时间关联不同任务，应使用 taskId。

**快照语义**：消息保存产生时的状态；回调中另行调用 snapshot() 可能读到更晚的状态。

**适用范围**：通知依赖对象所属线程的事件循环；退订后不再收到通知，析构路径也不承诺继续发送 Finished。

---

## 7. 委托与回调

### 7.1 TaskEventHandler

```cpp
using TaskEventHandler = std::function<void(const TaskEvent& event)>;
```

**说明**：统一接收 Started、Progress、Finished。消息定义“发生了什么”，委托定义“调用谁来接收”，不是两套重复的通知机制。

**参数**：

| 参数名 | 类型 | 说明 |
|------|------|------|
| `event` | `const TaskEvent&` | 引用仅在本次回调期间有效；需要保存时复制事件或快照 |

**返回值**：`void` — 不通过回调返回值控制任务；取消操作显式调用 cancel()。

**执行约束**：

| 事项 | 约定 |
|------|------|
| 执行线程 | 执行器所属线程，不是图像处理工作线程 |
| 调用方式 | 异步分发；同一执行器的委托调用不并发执行 |
| 耗时要求 | 不要阻塞或执行重计算，以免阻塞通知和界面事件循环 |
| 异常处理 | 捕获并记录回调异常，继续分发其他订阅；回调异常不改变任务结果，也不自动重试 |
| 允许的回调内调用 | snapshot()、cancel()、unsubscribe() |
| 重入限制 | 不在回调中启动新任务或销毁执行器；需要时延后到回调返回后处理 |
| 捕获对象 | 捕获的引用或指针必须有效；对象释放前取消订阅，或使用弱引用判断是否仍存在 |

---

## 8. 错误处理

| ErrorCode | 发生阶段 | 含义与处理 |
|------|------|------|
| `None` | 提交成功或无错误终态 | 无业务错误；取消用 TaskState::Cancelled 表示，不作为失败 |
| `Busy` | start() 拒绝 | 已有任务运行；等待终态再提交，不改变当前任务 |
| `InvalidRequest` | start() 拒绝 | 字段缺失、重复图片标识、非法路径/标识或不合法的滤镜参数 |
| `FilterNotFound` | start() 拒绝 | 所需内置或插件滤镜未注册；不静默跳过操作 |
| `ReadFailed` | 后台处理 | 输入文件不存在、不可读或无法解码；以 Failed 结束 |
| `ProcessFailed` | 后台处理 | 滤镜处理失败；以 Failed 结束 |
| `WriteFailed` | 后台处理 | 输出冲突、无写入权限或保存失败；以 Failed 结束 |
| `InternalError` | 提交或后台处理 | 其他内部故障；提交阶段不接受请求，运行阶段以 Failed 结束 |

**返回错误与异常**：业务失败使用 StartResult.error 或 Finished.snapshot.error；空委托等接口误用使用明确的 C++ 异常。分配失败等运行时异常可直接传播，启动阶段抛异常不视为任务被接受。

**失败后的状态**：任务失败时保留已成功输出的文件，completed 与 outputPaths 只统计成功结果；失败不会让整个批次自动回滚。内部日志记录定位信息，对外 message 不暴露密钥等敏感数据。

**调用方规则**：判断 code 和 state，不依赖 message 的具体文字。

---

## 9. 生命周期与线程安全

### 9.1 生命周期

```
准备滤镜注册表和编解码服务
        ↓
创建 VmBatchRunner → subscribe()
        ↓
start() → 接收通知 / snapshot() → 可选 cancel()
        ↓
收到 Finished → unsubscribe()
        ↓
回调返回后销毁执行器 → 再释放依赖服务和插件
```

### 9.2 状态与资源约束

| 事项 | 约定 |
|------|------|
| 运行状态 | Idle → Running → Succeeded / Failed / Cancelled；终态可以重新提交任务 |
| 取消请求 | cancelRequested 是标志，不额外创造一种任务运行状态 |
| 线程归属 | 构造、公开方法及析构均在所属线程；其他线程需投递到该线程调用 |
| 内部并发 | 工作线程只处理复制的请求和像素数据，状态由所属线程统一发布 |
| 服务所有权 | registry、codec 由调用方持有，执行器只借用；活动任务期间不得修改注册内容或卸载插件 |
| 请求所有权 | start() 接受后持有值副本，不继续借用原 BatchRequest |
| 订阅所有权 | 执行器持有委托；退订或析构时释放，捕获对象不会因此自动获得生命周期保证 |
| 对象销毁 | 析构先停止分发，再取消并等待工作线程，不要求工作线程等待 UI 回调才能退出 |
| Qt 父对象 | 使用父对象管理生命周期时，不再同时使用可能晚于父对象释放的独立拥有型智能指针 |

### 9.3 注意事项 / 已知限制

- 不支持强制打断单次解码、滤镜或写入；取消和析构等待时长受当前操作影响。
- 事件循环暂停时通知不会及时分发；不可在所属线程阻塞等待 Finished。
- 文件可能在 start() 返回后发生变化，读取与写入失败仍须通过异步结果处理。
- 方法与线程安全描述是 Demo 的目标契约，实际实现仍需单元测试和集成测试验证。
- 本 Demo 不承诺同一编解码服务被多个执行器并发使用；共享依赖时需另行明确其线程安全约束。

---

## 10. 依赖关系与设计说明

| 依赖项 | 类型 | 说明 |
|------|------|------|
| `Algo::FilterRegistry` | 内部依赖 | 按处理标识定位滤镜；注册与释放由应用装配和插件管理负责 |
| `Algo::ImageCodec` | 内部依赖 | 负责图片解码和结果保存，执行器不直接暴露 OpenCV 类型 |
| `Object::BatchTask` | 业务协作 | 由 VmTaskManager 转为 BatchRequest，并根据 TaskSnapshot 更新业务对象；工作线程不持有其可变引用 |
| `Common::AppLog` | 内部依赖 | 记录后台故障及回调异常 |
| Qt Core | 第三方库 | 提供对象线程归属与排队调度，不依赖 Qt Widgets / Gui |
| C++17 标准库 | 标准库 | 提供值类型、容器、委托及资源管理 |

**设计思路**：提交结果与执行结果分离；使用值快照跨越异步边界；所有通知使用同一种载荷和委托；调用方通过退订明确结束接收，避免把私有实现细节写成公开 API。

---

## 11. 调用示例

### 11.1 完整 Demo

本节是唯一的调用示例，将构造、订阅、启动、读取状态、取消、退订和析构串在同一程序中。方法章节不再重复放片段。

**示例前提**：需要按本文契约提供示例头文件和实现。Algo 的初始化接口约定为 `registerFilter(id, std::shared_ptr<IImageFilter>)`，用于注册滤镜，失败时抛出异常；它是依赖模块的初始化，不属于 VmBatchRunner 的公开方法。此处不是当前仓库可直接编译的业务程序。

**执行过程**：程序在自动清理的临时目录中生成一张 PPM 测试图片，再提交缩放任务。为展示 cancel()，提交后立即请求取消；小任务可能已完成，所以 Succeeded 和 Cancelled 都是合法结果。

```cpp
// 导入任务与滤镜接口
#include "ViewModel/VmBatchRunner.h"
#include "Algo/FilterRegistry.h"
#include "Algo/ImageCodec.h"
#include "Algo/ResizeFilter.h"

// 导入事件循环与文件工具
#include <QByteArray>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
// 导入临时目录与异常输出
#include <QTemporaryDir>
#include <exception>
#include <iostream>
#include <memory>
// 导入委托移动工具
#include <utility>

// 创建演示运行环境
int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);
    QTemporaryDir workspace;
    // 检查临时工作目录
    if (!workspace.isValid()) {
        std::cerr << "无法创建临时目录\n";
        return 1;
    }

    // 准备输入和输出路径
    const QString inputPath = workspace.path() + "/input.ppm";
    const QString outputDir = workspace.path() + "/output";
    // 创建输出目录
    if (!QDir().mkpath(outputDir)) {
        return 1;
    }

    // 生成测试图片像素
    QFile inputFile(inputPath);
    QByteArray pixels("P6\n2 2\n255\n");
    pixels += QByteArray::fromHex("ff0000" "00ff00" "0000ff" "ffffff");
    // 写入测试图片
    if (!inputFile.open(QIODevice::WriteOnly)
        || inputFile.write(pixels) != pixels.size()) {
        // 报告图片写入失败
        std::cerr << "无法生成测试图片\n";
        return 1;
    }
    inputFile.close();

    // 使用任务与算法命名空间
    using namespace ImageBatch::ViewModel;
    namespace Algo = ImageBatch::Algo;

    // 处理初始化与调用异常
    try {
        // 初始化滤镜和编解码服务
        Algo::FilterRegistry registry;
        Algo::ImageCodec codec;
        registry.registerFilter("resize", std::make_shared<Algo::ResizeFilter>());

        // 创建任务执行器并读取初始状态
        auto runner = std::make_unique<VmBatchRunner>(registry, codec);
        std::cout << "初始状态为 Idle: "
                  << (runner->snapshot().state == TaskState::Idle) << "\n";

        // 初始化任务通知处理
        bool terminalSeen = false;
        TaskEventHandler handler = [&](const TaskEvent& event) {
            const auto& status = event.snapshot;
            switch (event.kind) {
            // 显示开始通知
            case EventKind::Started:
                std::cout << "已开始: " << status.taskId << "\n";
                break;
            // 显示处理进度
            case EventKind::Progress:
                std::cout << "进度: " << status.completed
                          << "/" << status.total << "\n";
                break;
            // 记录任务完成结果
            case EventKind::Finished:
                terminalSeen = true;
                std::cout << "任务结束，错误信息: "
                          << status.error.message << "\n";
                // 结束事件循环
                app.quit();
                break;
            }
        };
        // 订阅任务通知
        const auto token = runner->subscribe(std::move(handler));

        // 设置任务标识和输入图片
        BatchRequest request;
        request.taskId = "demo-task";
        request.inputs.push_back({"image-a", inputPath.toUtf8().toStdString()});
        // 添加缩放操作与输出目录
        request.operations.push_back({
            "resize", Algo::FilterParams{{"width", 64}, {"height", 64}}
        });
        request.outputDirectory = outputDir.toUtf8().toStdString();

        // 提交批处理任务
        const auto submitted = runner->start(request);
        if (!submitted.accepted) {
            std::cerr << "提交失败: " << submitted.error.message << "\n";
            // 清理未被接收的任务
            runner->unsubscribe(token);
            runner.reset();
            return 1;
        }

        // 读取任务进度
        std::cout << "任务图片数: " << runner->snapshot().total << "\n";
        // 发送取消请求
        const bool cancellationRequested = runner->cancel();
        std::cout << "已登记取消请求: " << cancellationRequested << "\n";

        // 分发任务通知
        const int eventLoopCode = app.exec();

        // 保存结果并释放执行器
        const TaskSnapshot finalStatus = runner->snapshot();
        runner->unsubscribe(token);
        runner.reset();

        // 检查最终状态
        if (!terminalSeen || finalStatus.state == TaskState::Failed) {
            std::cerr << finalStatus.error.message << "\n";
            return 1;
        }
        // 输出处理结果
        std::cout << "成功输出文件数: " << finalStatus.outputPaths.size() << "\n";
        return eventLoopCode;
    } catch (const std::exception& error) {
        // 报告调用异常
        std::cerr << error.what() << "\n";
        return 1;
    }
}
```

### 11.2 方法调用覆盖

| 公开方法 | 完整 Demo 中的位置 |
|------|------|
| `VmBatchRunner(...)` | 使用 make_unique 创建执行器 |
| `subscribe(...)` | 提交任务前安装统一事件委托 |
| `start(...)` | 提交 BatchRequest，并先检查 accepted |
| `snapshot()` | 读取初始状态、提交后的进度信息和最终结果 |
| `cancel()` | 提交后立即请求取消，等待终态确认实际结果 |
| `unsubscribe(...)` | 拒绝提交或任务结束后取消订阅 |
| `~VmBatchRunner()` | 通过 runner.reset() 在回调返回后释放执行器 |

**使用提醒**：要观察正常完成流程，可以去掉 cancel() 调用。临时目录会在程序退出时删除，其中的图片和输出仅供本 Demo 演示。
