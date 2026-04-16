# macOS 官方壁纸管理 API 调查与 Rust 调用

## 结论

macOS **公开了桌面壁纸管理 API**，核心入口是 `AppKit` 框架里的 `NSWorkspace`。

公开 API 能做的事主要是：

- 读取指定屏幕当前桌面壁纸的 URL
- 读取指定屏幕当前桌面壁纸的显示选项
- 为指定屏幕设置新的桌面壁纸

公开 API 目前暴露的是“**按屏幕设置静态桌面图片**”这一层能力，不是一个完整的“壁纸库管理系统”API。根据当前公开文档可见范围，Apple 没有提供用于第三方 App 管理系统壁纸库、动态壁纸集合、桌面轮播计划、每个 Space 独立策略等更高层壁纸系统能力的公开 SDK 接口。

## 官方公开 API

### 1. `NSWorkspace.shared`

壁纸管理从共享工作区对象进入：

```swift
let workspace = NSWorkspace.shared
```

在 Rust 绑定里，对应 `NSWorkspace::sharedWorkspace()`。

### 2. `desktopImageURL(for:)`

读取某个 `NSScreen` 当前壁纸文件 URL：

```swift
func desktopImageURL(for screen: NSScreen) -> URL?
```

用途：

- 获取当前屏幕正在使用的壁纸文件
- 作为“先读后改”的基础能力

### 3. `desktopImageOptions(for:)`

读取当前屏幕壁纸的显示选项：

```swift
func desktopImageOptions(for screen: NSScreen) -> [NSWorkspace.DesktopImageOptionKey : Any]?
```

Apple 文档明确说明：**这个方法必须在主线程调用。**

### 4. `setDesktopImageURL(_:for:options:)`

设置某个 `NSScreen` 的桌面壁纸：

```swift
func setDesktopImageURL(
    _ url: URL,
    for screen: NSScreen,
    options: [NSWorkspace.DesktopImageOptionKey : Any]
) throws
```

这是公开 API 中最关键的方法。

用途：

- 把某张本地图片设为指定屏幕的桌面壁纸
- 搭配 `options` 控制缩放、裁剪和填充色

### 5. `NSWorkspace.DesktopImageOptionKey`

Apple 公开了桌面图片选项键，至少包括这几个常用项：

- `NSWorkspaceDesktopImageScalingKey`
- `NSWorkspaceDesktopImageAllowClippingKey`
- `NSWorkspaceDesktopImageFillColorKey`

这些键用于 `setDesktopImageURL(_:for:options:)` 和 `desktopImageOptions(for:)` 的字典。

常见含义：

- `ScalingKey`：控制图片缩放方式
- `AllowClippingKey`：是否允许裁剪
- `FillColorKey`：空白区域填充颜色

文档页把这些键定义为 `NSWorkspace.DesktopImageOptionKey`。

## 能力边界

当前公开 SDK 能确认的边界如下。

### 可以做

- 按 `NSScreen` 读取当前壁纸
- 按 `NSScreen` 设置壁纸
- 控制基础显示选项

### 不能从公开 API 直接做

- 管理系统“壁纸图库”或系统壁纸资源库
- 操作桌面与屏保设置面板中的完整 UI 状态
- 管理动态壁纸的系统级元数据与切换策略
- 控制桌面轮播/自动切换计划的完整系统行为
- 以公开 API 的方式精细管理每个 Space 的独立壁纸策略

上面“不能直接做”的结论，是基于当前 Apple 公开 AppKit 文档中可见接口范围得出的推断；如果要达到这些能力，通常只能依赖私有接口、UI 自动化、AppleScript/Finder 边缘方案，或 MDM 级设备管理配置，而不是普通应用 SDK。

## Rust 中如何调用

Rust 里最稳妥的方式是通过 Objective-C 绑定调用 AppKit。当前更推荐使用 `objc2` 系列，而不是老的 `cocoa` crate。

建议依赖：

```toml
[dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["NSString", "NSURL", "NSDictionary"] }
objc2-app-kit = { version = "0.3", features = ["NSWorkspace", "NSScreen", "NSColor"] }
```

如果你后面要更完整地构造 App 生命周期，可能还需要加 `NSApplication` 等 feature；但仅做壁纸读写，以上依赖通常足够说明接口用法。

## Rust 最小示例

下面示例展示：

- 获取主屏幕
- 读取当前壁纸 URL
- 将指定本地图片设为壁纸
- 设置一个空的 `options` 字典

说明：下面代码用于展示调用骨架。`objc2` 不同小版本的泛型和返回类型细节可能略有差异，但核心调用路径就是 `NSWorkspace -> NSScreen -> setDesktopImageURL`。

```rust
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSScreen, NSWorkspace};
use objc2_foundation::{NSDictionary, NSURL};

fn main() {
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();

        let screen = NSScreen::mainScreen().expect("No main screen found");

        if let Some(current_url) = workspace.desktopImageURLForScreen(&screen) {
            println!("Current wallpaper URL: {:?}", current_url);
        }

        let image_url = NSURL::fileURLWithPath("/Users/your-name/Pictures/wallpaper.jpg");

        let options = NSDictionary::new();

        unsafe {
            workspace
                .setDesktopImageURL_forScreen_options_error(&image_url, &screen, &options)
                .expect("Failed to set desktop wallpaper");
        }
    });
}
```

## 关于 `options`

最简单的调用可以先传空字典：

```rust
let options = NSDictionary::new();
```

如果要保留系统当前显示选项，一个更稳的做法是先读取后复用：

```rust
let options = workspace
    .desktopImageOptionsForScreen(&screen)
    .unwrap_or_else(NSDictionary::new);
```

这样做的好处是：

- 不需要自己猜缩放参数
- 更接近“只替换图片，不改显示方式”

## Rust 调用时的注意点

### 1. 仅适用于 macOS

这套接口来自 `AppKit`，只能在 macOS 下使用。

### 2. 建议在主线程调用

Apple 对 `desktopImageOptions(for:)` 明确要求主线程调用。考虑到这里同时涉及 `AppKit`、`NSScreen` 和桌面 UI 状态，实际工程里应把读取与设置壁纸都放在主线程执行。

### 3. 图片 URL 必须是本地可读文件

`setDesktopImageURL` 接收的是文件 URL。也就是说：

- 你通常要先把图片下载到本地
- 然后传 `file://` 对应的 `NSURL`

### 4. 沙盒应用要有文件读取权限

如果你的 App 是 sandboxed macOS App，那么它不仅要能调用该 API，还必须对目标图片文件拥有读取权限；否则即使接口本身公开，也会因为文件访问受限而失败。

### 5. 错误处理不能省略

Rust 绑定里 `setDesktopImageURL_forScreen_options_error` 是 `unsafe`，并返回 `Result<(), NSError>`。这里至少要处理：

- 路径不存在
- 文件不是有效图片
- 文件没有访问权限
- 屏幕对象不可用

## 一个更实用的 Rust 封装示例

```rust
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSScreen, NSWorkspace};
use objc2_foundation::{NSDictionary, NSURL};

pub fn set_wallpaper(path: &str) -> Result<(), String> {
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let screen = NSScreen::mainScreen().ok_or_else(|| "No main screen found".to_string())?;
        let image_url = NSURL::fileURLWithPath(path);
        let options = workspace
            .desktopImageOptionsForScreen(&screen)
            .unwrap_or_else(NSDictionary::new);

        unsafe {
            workspace
                .setDesktopImageURL_forScreen_options_error(&image_url, &screen, &options)
                .map_err(|err| format!("Failed to set wallpaper: {:?}", err))?;
        }

        Ok(())
    })
}
```

调用：

```rust
fn main() {
    if let Err(err) = set_wallpaper("/Users/your-name/Pictures/wallpaper.jpg") {
        eprintln!("{err}");
    }
}
```

## 适合你的项目的落地建议

如果你的目标是做一个“智能壁纸”应用，建议把能力拆成两层：

### 官方 API 层

只负责：

- 读当前壁纸
- 设新壁纸
- 保留或调整显示选项

### 业务层

自己实现：

- 壁纸下载与缓存
- 壁纸库索引
- 定时轮换
- 多屏策略
- 标签与推荐逻辑

也就是说，macOS 官方公开的部分更像“**最终落盘到系统桌面**”的输出接口，而不是完整的壁纸管理平台。

## 参考资料

- Apple Developer: `NSWorkspace`
  - https://developer.apple.com/documentation/appkit/nsworkspace
- Apple Developer: `desktopImageOptions(for:)`
  - https://developer.apple.com/documentation/appkit/nsworkspace/desktopimageoptions%28for%3A%29
- Apple Developer: `desktopImageURL(for:)`
  - https://developer.apple.com/documentation/appkit/nsworkspace/1530635-desktopimageurlforscreen
- Apple Developer: `setDesktopImageURL(_:for:options:)`
  - https://developer.apple.com/documentation/appkit/nsworkspace/setdesktopimageurl%28_%3Afor%3Aoptions%3A%29
- Apple Developer: `NSWorkspace.DesktopImageOptionKey`
  - https://developer.apple.com/documentation/appkit/nsworkspace/desktopimageoptionkey
- Apple Developer: `NSWorkspaceDesktopImageScalingKey`
  - https://developer.apple.com/documentation/appkit/nsworkspacedesktopimagescalingkey
- Apple Developer: `NSWorkspaceDesktopImageAllowClippingKey`
  - https://developer.apple.com/documentation/appkit/nsworkspacedesktopimageallowclippingkey
- Apple Developer: `NSWorkspaceDesktopImageFillColorKey`
  - https://developer.apple.com/documentation/appkit/nsworkspacedesktopimagefillcolorkey
- docs.rs: `objc2-app-kit::NSWorkspace`
  - https://docs.rs/objc2-app-kit/latest/objc2_app_kit/struct.NSWorkspace.html
- docs.rs source: `NSWorkspaceDesktopImageOptionKey` 与 `setDesktopImageURL_forScreen_options_error`
  - https://docs.rs/objc2-app-kit/latest/aarch64-apple-ios-macabi/src/objc2_app_kit/generated/NSWorkspace.rs.html
