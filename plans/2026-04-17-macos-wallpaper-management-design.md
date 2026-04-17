# macOS 壁纸管理模块设计

## 1. 背景与目标

当前项目是一个 Rust 应用，目标是在 macOS 上提供“壁纸管理”能力。这里的“管理”不是单次设置图片，而是抽象出一个专门的模块，负责：

- 读取当前壁纸状态
- 设置指定屏幕的壁纸
- 批量设置多屏壁纸
- 维护壁纸展示选项
- 处理系统屏幕变更与异常

该模块应当独立、可测试、可替换底层实现，并为后续扩展动态壁纸、定时轮换、智能推荐提供稳定边界。

## 2. Apple 官方能力边界

基于 Apple 官方 AppKit 文档，macOS 原生壁纸能力主要来自 `NSWorkspace` 与 `NSScreen`：

- `NSScreen.screens`：获取当前所有屏幕
- `NSWorkspace.desktopImageURL(for:)`：读取某个屏幕当前壁纸
- `NSWorkspace.desktopImageOptions(for:)`：读取某个屏幕的壁纸展示选项
- `NSWorkspace.setDesktopImageURL(_:for:options:)`：为某个屏幕设置壁纸

已知约束：

- 壁纸操作绑定 `NSScreen`
- 屏幕列表不可长期缓存，显示器配置会动态变化
- `desktopImageOptions(for:)` 需在主线程调用
- 壁纸设置失败会抛错，必须显式处理
- 系统 API 接受的是文件 URL，不能假定任意来源数据都可直接设置
- 系统能否成功展示图片，最终仍取决于 AppKit / ImageIO 对该文件的解码能力

来源：

- https://developer.apple.com/documentation/appkit/nsworkspace
- https://developer.apple.com/documentation/appkit/nsscreen
- https://developer.apple.com/documentation/appkit/nsworkspace/desktopimageoptions%28for%3A%29

## 3. 需求摘要

### 功能需求

1. 枚举当前所有屏幕及其标识
2. 查询每个屏幕当前壁纸路径与展示模式
3. 为单屏设置壁纸
4. 为多屏分别设置壁纸
5. 支持“统一设置全部屏幕”
6. 在屏幕插拔后刷新状态
7. 对非法路径、权限错误、格式错误给出明确错误

### 非功能需求

- 模块边界清晰，不与 UI、调度、推荐逻辑耦合
- 底层 macOS API 可替换
- 主线程约束由模块内部处理，不外泄给业务层
- 状态读取与写入具备幂等性和可观测性
- 为未来缓存、历史记录、轮换策略预留扩展点
- 对失败场景的行为可预测，不能出现“系统设置失败但本地状态误认为成功”

## 4. 方案比较

### 方案 A：纯 Rust 直接桥接 AppKit

做法：通过 `objc2` / `objc2-app-kit` 直接调用 `NSWorkspace`、`NSScreen`。

优点：

- 全部逻辑保持在 Rust 内
- 部署结构简单，无额外 helper

缺点：

- Objective-C/AppKit 桥接复杂
- 主线程与 RunLoop 管理容易踩坑
- 对 AppKit 字典参数、错误桥接、屏幕标识映射实现成本较高

适用：团队熟悉 Rust + Cocoa FFI，且希望把系统集成完全收敛到 Rust。

### 方案 B：Rust 领域层 + Swift 原生桥接层

做法：Rust 定义壁纸管理抽象，Swift 小型桥接层负责直接调用 `NSWorkspace`。

优点：

- 与 Apple API 贴合，线程模型清晰
- 壁纸系统适配层更稳定
- Rust 保持核心业务与状态编排

缺点：

- 构建链路更复杂
- 需要定义 Rust/Swift 边界

适用：需要长期演进，且希望降低 AppKit 桥接风险。

### 方案 C：AppleScript / shell 封装

做法：通过脚本或 `osascript` 设置壁纸。

优点：

- 原型快

缺点：

- 可维护性差
- 错误语义弱
- 多屏支持、状态读取和长期扩展都较差

适用：仅做临时 PoC，不适合正式模块。

### 推荐

推荐 **方案 B：Rust 领域层 + Swift 原生桥接层**。

原因：当前项目主体是 Rust，但 macOS 壁纸 API 位于 AppKit，且存在主线程限制。把“系统 API 接入”压缩到一层很薄的 Swift bridge，能显著降低 Rust FFI 复杂度，同时保留 Rust 在状态管理、任务编排、缓存、测试上的优势。这是当前阶段最稳妥的设计。

## 5. 模块边界与职责

建议引入独立模块：`wallpaper_manager`

职责只包含系统壁纸管理，不负责：

- 图片下载
- 智能推荐
- UI 展示
- 轮换调度策略

这些能力都应依赖 `wallpaper_manager`，而不是反向耦合进去。

### 模块对外职责

- 暴露统一的查询/设置接口
- 维护屏幕与壁纸状态快照
- 屏蔽底层平台调用细节
- 统一错误模型
- 响应系统显示器配置变化

### 模块边界之外

以下能力不应进入 `wallpaper_manager` 内部：

- 下载远程图片并落盘
- 图片编辑、裁剪、滤镜、格式转换
- 生成缩略图
- 媒体资源库管理
- 用户交互确认逻辑

这些能力可以在上层完成，然后把一个已经准备好的本地图片文件路径交给 `wallpaper_manager`。

## 6. 推荐架构

```text
+---------------------------+
| UI / Command / Scheduler  |
+-------------+-------------+
              |
              v
+---------------------------+
| WallpaperApplicationService |
+-------------+-------------+
              |
      +-------+--------+
      |                |
      v                v
+-------------+  +------------------+
| State Store  |  | WallpaperBackend |
+-------------+  +------------------+
                        |
                        v
               +-------------------+
               | Swift AppKit Bridge |
               +-------------------+
                        |
                        v
               +-------------------+
               | NSWorkspace/NSScreen |
               +-------------------+
```

## 7. 模块内部设计

### 7.1 领域模型

```rust
pub struct ScreenId(String);

pub enum WallpaperScaling {
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
}

pub struct WallpaperOptions {
    pub scaling: WallpaperScaling,
    pub allow_clipping: bool,
}

pub struct WallpaperAssignment {
    pub screen_id: ScreenId,
    pub image_path: PathBuf,
    pub options: WallpaperOptions,
}

pub struct WallpaperState {
    pub screen_id: ScreenId,
    pub image_path: Option<PathBuf>,
    pub options: WallpaperOptions,
}

pub struct ScreenDescriptor {
    pub screen_id: ScreenId,
    pub localized_name: String,
    pub is_builtin: bool,
    pub is_primary: bool,
    pub frame: ScreenFrame,
    pub native_size: ScreenSize,
    pub vendor_id: Option<u32>,
    pub model_id: Option<u32>,
    pub serial_number: Option<u32>,
}
```

### 7.2 核心接口

```rust
pub trait WallpaperBackend {
    fn list_screens(&self) -> Result<Vec<ScreenDescriptor>, WallpaperError>;
    fn get_wallpaper(&self, screen_id: &ScreenId) -> Result<WallpaperState, WallpaperError>;
    fn set_wallpaper(&self, assignment: &WallpaperAssignment) -> Result<(), WallpaperError>;
}

pub struct WallpaperManager<B: WallpaperBackend> {
    backend: B,
    store: WallpaperStateStore,
}

impl<B: WallpaperBackend> WallpaperManager<B> {
    pub fn refresh(&mut self) -> Result<Vec<WallpaperState>, WallpaperError>;
    pub fn set_for_screen(&mut self, assignment: WallpaperAssignment) -> Result<(), WallpaperError>;
    pub fn set_for_all(&mut self, image: PathBuf, options: WallpaperOptions) -> Result<(), WallpaperError>;
    pub fn set_batch(&mut self, assignments: Vec<WallpaperAssignment>) -> Result<(), WallpaperError>;
}
```

### 7.3 分层职责

- `WallpaperManager`：应用服务，负责流程编排、批量操作、状态同步
- `WallpaperBackend`：平台抽象层，隔离 macOS 调用细节
- `SwiftAppKitBridgeBackend`：推荐的 macOS 实现
- `WallpaperStateStore`：保存最近一次同步的壁纸状态快照
- `DisplayWatcher`：监听屏幕拓扑变更并触发刷新

### 7.4 输入边界与校验职责

建议把输入校验拆成三层：

1. 请求层校验
   校验 `screen_id` 是否存在、请求是否为空、批量请求是否存在重复目标屏幕
2. 文件层校验
   校验路径存在、可读、是普通文件而不是目录、符号链接目标有效
3. 媒体层校验
   校验扩展名、MIME 推断结果、图片是否可被系统解码

职责分配：

- `WallpaperManager` 负责请求层校验和流程编排
- `WallpaperBackend` 负责系统可执行性校验与最终设置
- 上层业务负责“图片从哪里来”和“是否需要事先转码”

## 8. 屏幕标识设计

这是实现成败的关键点之一。

`NSScreen` 对象本身不能直接作为跨层稳定标识。建议在 bridge 层提取可稳定映射的显示器标识，并向 Rust 返回：

- `screen_uuid` 或系统显示器唯一标识
- `localized_name`
- 分辨率
- 是否主屏

Rust 层统一使用 `ScreenId`，禁止业务层直接处理 `NSScreen` 引用。

如果系统 API 无法稳定给出 UUID，则采用组合键：

`vendor_id + model_id + serial_number + frame`

并在文档中明确这属于“尽最大努力”的稳定映射。

### 多个外接屏幕下的标识策略

必须显式支持以下场景：

- 内建屏 + 1 个外接屏
- 内建屏 + 2 到 4 个外接屏
- 两个同品牌同型号外接屏
- 扩展坞断开再重连
- 用户调整主屏或重新排列显示器位置

设计原则：

- 业务层绝不能依赖 `NSScreen.screens` 返回顺序
- 业务层绝不能使用“第 1 块屏”“第 2 块屏”作为持久映射键
- `frame` 只能用于辅助匹配，不能单独作为稳定标识

推荐分两层标识：

1. `screen_id`
   用于当前运行期内的稳定引用，绑定到当前拓扑中的一个屏幕
2. `display_fingerprint`
   用于跨插拔、跨重启的尽力识别，建议由以下字段组合：
   `vendor_id + model_id + serial_number + is_builtin`

当 `serial_number` 缺失且存在多个同型号外接屏时，仅靠硬件信息无法完全区分。此时需要降级策略：

- 优先使用最近一次已知 `frame/origin`
- 如果仍无法唯一匹配，则将该屏幕标记为“ambiguous”
- 禁止把旧的壁纸映射自动应用到不确定目标

这条规则很关键。宁可要求用户重新确认，也不要把壁纸设到错误的显示器上。

### 建议新增的领域模型

```rust
pub struct DisplayFingerprint {
    pub vendor_id: Option<u32>,
    pub model_id: Option<u32>,
    pub serial_number: Option<u32>,
    pub is_builtin: bool,
}

pub enum ScreenMatchQuality {
    Exact,
    Fuzzy,
    Ambiguous,
}
```

## 9. 数据流

### 读取流程

1. 上层调用 `WallpaperManager.refresh()`
2. `WallpaperBackend.list_screens()` 返回当前屏幕集合
3. 逐屏调用 `get_wallpaper`
4. 结果写入 `WallpaperStateStore`
5. 返回统一状态快照

### 单屏设置流程

1. 校验图片路径存在且可读
2. 校验图片格式是否在允许范围内
3. 校验 `screen_id` 当前仍存在
4. 调用 backend 执行主线程壁纸设置
5. 成功后重新读取该屏幕壁纸状态
6. 只有读取结果与预期一致时才更新本地快照
7. 发布成功或失败事件

### 多屏批量设置流程

建议按“逐屏提交 + 收集结果”实现，而不是事务化全回滚。

原因：

- macOS 壁纸设置本身不是事务系统
- 回滚会引入额外状态复杂度
- 用户更需要知道“哪块屏成功、哪块屏失败”

返回值应包含逐屏结果。

### 写入后确认策略

为避免“API 调用返回成功，但实际状态未生效”的伪成功，建议所有写操作都遵循：

1. 执行 `setDesktopImageURL`
2. 立即调用 `desktopImageURL(for:)` 读取当前值
3. 校验读取结果是否与目标路径一致
4. 一致才提交本地状态，不一致则记为失败

这会略微增加一次系统调用，但能显著提升状态一致性。

### 多外接屏场景下的刷新流程

当系统显示器配置发生变化时，`DisplayWatcher` 应触发一次全量刷新，而不是尝试做局部修补。

推荐流程：

1. 重新枚举当前所有屏幕
2. 为每个屏幕重新生成 `screen_id` 与 `display_fingerprint`
3. 与上一次快照做匹配
4. 输出以下事件分类：
   - `screen_added`
   - `screen_removed`
   - `screen_reidentified`
   - `screen_ambiguous`
5. 对可精确匹配的屏幕恢复状态关联
6. 对模糊或冲突屏幕只保留“当前读取状态”，不自动继承旧绑定

这样可以避免扩展坞重连或显示器重新排序后，把 A 屏的壁纸误写到 B 屏。

### 多屏设置策略

针对多个外接屏幕，模块应支持三种设置模式：

1. `ApplyToScreen`
   只设置一个指定屏幕
2. `ApplyToAllScreens`
   把同一张图设置到当前全部屏幕
3. `ApplyPerScreen`
   为每个屏幕分别指定图片和选项

建议在应用服务层建模：

```rust
pub enum WallpaperSetRequest {
    ApplyToScreen(WallpaperAssignment),
    ApplyToAllScreens {
        image_path: PathBuf,
        options: WallpaperOptions,
    },
    ApplyPerScreen(Vec<WallpaperAssignment>),
}
```

其中 `ApplyPerScreen` 是多外接屏场景的核心接口。后续无论是“每个屏幕不同图片”，还是“给竖屏和横屏不同适配策略”，都应落到这条路径上。

### 分辨率与方向差异处理

多个外接屏往往存在以下差异：

- 分辨率不同
- 缩放比例不同
- 横屏与竖屏混用
- 内建 Retina 屏与普通显示器混用

因此设计上不能把“设置壁纸”简单理解为路径写入。建议在设置前增加一个轻量预检：

- 读取目标屏幕尺寸与方向
- 校验图片是否可用于目标屏幕
- 记录推荐的 `WallpaperScaling`

注意：

- 预检失败不一定阻止设置
- 但应返回 warning，便于 UI 提示“该图片在竖屏上可能被严重裁切”

### 图片格式策略

`wallpaper_manager` 应只接受“本地可读且系统可解码”的静态图片文件。

第一阶段建议白名单格式：

- `jpg`
- `jpeg`
- `png`
- `heic`
- `heif`
- `tiff`
- `bmp`

说明：

- 这是一层工程白名单，不等同于系统理论支持全集
- `gif` 不建议在第一阶段支持，因为即便可解码，也容易引入“取首帧还是动画”的语义歧义
- `webp` 是否支持不应靠扩展名假设，除非 bridge 层已经验证当前系统解码能力
- 动态壁纸包、视频、Live Photo 资源不应纳入当前模块输入范围

建议采用“双重判断”：

1. Rust 层基于扩展名做快速失败
2. Swift bridge 层使用系统解码能力做最终确认

只有两层都通过，才进入设置流程。

### 图片路径边界

以下路径情况需要明确处理：

- 文件不存在
- 路径指向目录
- 无读权限
- 符号链接存在但目标失效
- 文件正在下载或写入中
- 网络挂载卷文件暂时不可访问
- 路径包含空格、中文、emoji

设计原则：

- 使用文件 URL，不做 shell 转义式拼接
- 路径编码问题由 URL / `PathBuf` 处理，不手写字符串拼接
- 对“存在但暂时不可读”的情况单独归类，不混淆为格式错误

### 批量设置的回退与恢复策略

批量设置不做事务回滚，但要定义清晰的失败恢复语义。

推荐策略：

- 单屏失败不影响其他屏继续执行
- 成功屏的状态立即确认并提交
- 失败屏保留原系统状态，不写入伪新值
- 返回 `BatchSetReport` 让上层决定是否提示重试

不要做的事：

- 不要在第 3 块屏失败后回滚前 2 块屏
- 不要在无法确认原始旧壁纸时尝试“猜测性恢复”
- 不要用本地缓存覆盖系统真实状态

### 单屏设置失败时的回退策略

单屏设置的回退不是“把旧壁纸重新设回去”，而是：

1. 记录本次目标路径和错误
2. 重新读取该屏当前系统壁纸
3. 用读取到的真实状态覆盖本地快照
4. 把失败结果返回上层

原因：

- 调用失败时，系统可能完全未修改
- 也可能进入中间态后被系统自行修正
- 最可靠的恢复动作是重新读，而不是盲目回写旧值

### 拓扑变化中的失败处理

如果在批量设置过程中发生外接屏插拔或主屏切换，建议按以下规则处理：

- 当前正在设置的屏幕如果仍可定位，继续尝试一次
- 后续尚未执行的 assignment 重新校验 `screen_id`
- 对已失效目标直接返回 `ScreenTopologyChanged` 或 `ScreenNotFound`
- 整批任务结束后强制执行一次 `refresh()`

这样可以保证批处理结束时，本地快照至少与“当前拓扑”一致。

## 10. 错误模型

```rust
pub enum WallpaperError {
    ScreenNotFound(ScreenId),
    InvalidImagePath(PathBuf),
    UnsupportedImageFormat(PathBuf),
    PermissionDenied(PathBuf),
    ImageDecodeFailed(PathBuf),
    DuplicateScreenAssignment(ScreenId),
    EmptyBatchRequest,
    MainThreadViolation,
    PlatformApiError(String),
    ScreenTopologyChanged,
    AmbiguousScreenMapping,
    ReadAfterWriteMismatch {
        screen_id: ScreenId,
        expected: PathBuf,
        actual: Option<PathBuf>,
    },
    PartialFailure(Vec<ScreenOperationError>),
}
```

错误处理原则：

- 领域层使用稳定错误枚举
- bridge 层保留原始系统错误文本，便于排障
- 批量操作允许部分成功
- 屏幕拓扑变化期间的失败单独归类，避免误判为普通设置失败
- 对无法唯一定位目标外接屏的情况单独报错，不做猜测式写入
- 对“扩展名合法但系统无法解码”的情况单独归类
- 对“写入成功但读回校验失败”的情况单独归类

### 推荐错误分层

建议把错误按来源分三类理解：

1. 请求错误
   如空批量请求、重复屏幕 assignment、无效路径
2. 资源错误
   如权限不足、图片损坏、格式不支持、解码失败
3. 平台错误
   如主线程问题、屏幕拓扑变化、AppKit 返回错误、写后读不一致

这样上层 UI 可以更准确地决定：

- 是否提示用户修正输入
- 是否建议转码图片
- 是否建议刷新屏幕状态后重试

## 11. 并发与线程模型

AppKit 调用必须被视为 UI/Main Thread 敏感操作。

设计要求：

- 所有 `NSWorkspace` 交互由 Swift bridge 串行化
- bridge 内部保证主线程执行
- Rust 上层不直接关心线程切换
- 禁止多个并发写请求直接冲击系统 API

建议：

- 在 `WallpaperManager` 内部引入串行命令队列
- 读操作可缓存，但屏幕变更后必须失效
- 写操作完成后立即刷新对应状态
- 当 `refresh()` 正在执行时，新写请求应排队，而不是与刷新并行交错

### 状态一致性原则

本模块应遵守以下状态原则：

- 系统状态是真实来源，本地 store 只是快照
- 本地 store 只能由“成功读取系统状态”来更新
- 任何失败路径都不能直接伪造新状态写入 store
- 当出现不确定情况时，优先触发重新读取，而不是维持乐观缓存

这能避免模块随着失败重试逐渐积累错误状态。

## 12. 可观测性

建议该模块输出结构化日志：

- `wallpaper.refresh.started`
- `wallpaper.refresh.completed`
- `wallpaper.set.started`
- `wallpaper.set.succeeded`
- `wallpaper.set.failed`
- `wallpaper.display.changed`

关键字段：

- `screen_id`
- `image_path`
- `duration_ms`
- `error_code`
- `attempt_id`
- `topology_version`
- `match_quality`

这会显著降低后续排查“设置成功但界面未同步”或“多屏部分失败”的成本。

建议补充 warning 事件：

- `wallpaper.image.validation_warning`
- `wallpaper.read_after_write_mismatch`
- `wallpaper.screen.ambiguous`

## 13. 测试设计

### 单元测试

针对 Rust 领域层，使用 `MockWallpaperBackend` 测试：

- 单屏设置成功
- 屏幕不存在
- 批量设置部分成功
- 刷新时屏幕列表变化
- 状态缓存更新正确
- 空批量请求
- 同一屏幕被重复 assignment
- 合法扩展名但解码失败
- 写后读不一致时不更新 store
- 失败后通过重新读取恢复真实状态

### 集成测试

针对 macOS 真机或 CI Runner：

- 读取当前屏幕列表
- 设置测试图片到指定屏幕
- 再次读取，校验壁纸路径变化

多外接屏专项用例建议覆盖：

- 内建屏 + 单外接屏
- 内建屏 + 双外接屏
- 两个同型号外接屏并存
- 断开一个外接屏后刷新状态
- 重新插入扩展坞后做屏幕重识别
- `ApplyToAllScreens` 成功
- `ApplyPerScreen` 部分成功、部分失败
- HEIC / PNG / JPEG 实际可设置
- 不支持格式被提前拒绝
- 外接屏在设置过程中断开
- 网络卷图片短暂不可读时返回稳定错误

说明：

真实壁纸设置属于系统副作用，不适合高频 CI；建议把这类测试标记为 `ignored` 或单独的本地集成测试集。

## 14. ADR

### ADR-001：壁纸管理使用独立模块

- 决策：创建 `wallpaper_manager` 独立模块
- 原因：壁纸管理属于平台能力，需与 UI、推荐、调度解耦
- 影响：后续功能通过模块接口接入，避免系统 API 扩散到全项目

### ADR-002：采用 Rust + Swift bridge，而非纯脚本

- 决策：底层通过 Swift bridge 调用 AppKit
- 原因：兼顾 Rust 主体架构与 Apple API 原生适配
- 备选：纯 Rust FFI、AppleScript
- 取舍：牺牲少量构建复杂度，换取更低系统集成风险

### ADR-003：批量设置采用部分成功模型

- 决策：批量操作返回逐屏结果，不做全局事务回滚
- 原因：系统 API 不具备事务能力，强行回滚复杂且收益低
- 影响：上层 UI 需呈现逐屏结果

### ADR-004：写入后必须读回确认

- 决策：每次设置壁纸后都执行一次读回校验
- 原因：避免系统调用成功但本地状态与真实状态不一致
- 代价：增加一次读取调用
- 收益：显著提高故障定位与状态正确性

### ADR-005：失败恢复以重新读取为主，不做盲目回写

- 决策：设置失败后优先 refresh / reread，不自动尝试设回旧壁纸
- 原因：失败时真实系统状态不一定等于旧缓存状态
- 影响：恢复策略更保守，但一致性更高

## 15. 推荐目录结构

```text
src/
  wallpaper_manager/
    mod.rs
    application.rs
    domain.rs
    error.rs
    store.rs
    backend.rs
    display_watcher.rs
    bridge/
      mod.rs
      swift_backend.rs
```

如果后续引入 Swift 独立 target，可演进为：

```text
native/
  WallpaperBridge/
    Sources/
      WallpaperBridge/
```

## 16. 第一阶段落地范围

建议第一阶段只做最小可用闭环：

1. 枚举屏幕
2. 获取当前壁纸
3. 设置单屏壁纸
4. 设置全部屏幕统一壁纸
5. 统一错误模型
6. 正确处理内建屏 + 多个外接屏的枚举与识别
7. 支持白名单图片格式校验
8. 设置后读回确认
9. 失败后按真实状态刷新快照

先不要在第一阶段引入：

- 动态壁纸
- 智能轮换
- 历史版本恢复
- 下载与素材管理
- 基于模糊匹配的自动恢复策略
- 自动图片转码
- 网络资源直连设置
- 动画图片语义支持

这部分应在 `wallpaper_manager` 稳定后再叠加。

## 17. 结论

最合适的方案是把 macOS 壁纸能力收敛为一个专门的 `wallpaper_manager` 模块，由 Rust 管理领域模型、状态和流程，由 Swift bridge 负责与 `NSWorkspace` / `NSScreen` 交互。这样可以把 Apple 平台细节局部化，降低线程与 FFI 风险，同时为后续扩展智能壁纸能力保留稳定架构边界。
