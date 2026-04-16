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
2. 校验 `screen_id` 当前仍存在
3. 调用 backend 执行主线程壁纸设置
4. 成功后刷新对应屏幕状态
5. 写入状态快照并发布事件

### 多屏批量设置流程

建议按“逐屏提交 + 收集结果”实现，而不是事务化全回滚。

原因：

- macOS 壁纸设置本身不是事务系统
- 回滚会引入额外状态复杂度
- 用户更需要知道“哪块屏成功、哪块屏失败”

返回值应包含逐屏结果。

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

## 10. 错误模型

```rust
pub enum WallpaperError {
    ScreenNotFound(ScreenId),
    InvalidImagePath(PathBuf),
    UnsupportedImageFormat(PathBuf),
    PermissionDenied(PathBuf),
    MainThreadViolation,
    PlatformApiError(String),
    ScreenTopologyChanged,
    AmbiguousScreenMapping,
    PartialFailure(Vec<ScreenOperationError>),
}
```

错误处理原则：

- 领域层使用稳定错误枚举
- bridge 层保留原始系统错误文本，便于排障
- 批量操作允许部分成功
- 屏幕拓扑变化期间的失败单独归类，避免误判为普通设置失败
- 对无法唯一定位目标外接屏的情况单独报错，不做猜测式写入

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

这会显著降低后续排查“设置成功但界面未同步”或“多屏部分失败”的成本。

## 13. 测试设计

### 单元测试

针对 Rust 领域层，使用 `MockWallpaperBackend` 测试：

- 单屏设置成功
- 屏幕不存在
- 批量设置部分成功
- 刷新时屏幕列表变化
- 状态缓存更新正确

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

先不要在第一阶段引入：

- 动态壁纸
- 智能轮换
- 历史版本恢复
- 下载与素材管理
- 基于模糊匹配的自动恢复策略

这部分应在 `wallpaper_manager` 稳定后再叠加。

## 17. 结论

最合适的方案是把 macOS 壁纸能力收敛为一个专门的 `wallpaper_manager` 模块，由 Rust 管理领域模型、状态和流程，由 Swift bridge 负责与 `NSWorkspace` / `NSScreen` 交互。这样可以把 Apple 平台细节局部化，降低线程与 FFI 风险，同时为后续扩展智能壁纸能力保留稳定架构边界。
