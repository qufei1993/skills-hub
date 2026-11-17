import AppKit
import Foundation

// MARK: - 参数
let templateCanvas: CGFloat = 1024
let templateInset: CGFloat = 100  // Apple 模板：每边 100pt 安全边距
let artworkInset: CGFloat = 48    // 原 SVG 底板 inset: x=48, y=48, 928x928

// 原 SVG 设计空间 (1024 - 48*2) = 928pt，映射到 Apple 模板的 824pt
let artworkScale = (templateCanvas - templateInset * 2) / (templateCanvas - artworkInset * 2)
let drawSize = templateCanvas * artworkScale
let drawOrigin = templateInset - artworkInset * artworkScale

print("artworkScale = \(artworkScale)")
print("drawSize = \(drawSize)pt")
print("drawOrigin = (\(drawOrigin), \(drawOrigin))pt")

// MARK: - 1. 读取源 PNG（icon-source.png, 1024x1024）
let sourcePath = CommandLine.arguments[1]
let outputPath = CommandLine.arguments[2]

guard let sourceImage = NSImage(contentsOfFile: sourcePath) else {
    fputs("Failed to load source image: \(sourcePath)\n", stderr)
    exit(1)
}

// MARK: - 2. 放大到 2048px (Retina 2× 超采样) 作为原始绘制图
let supersampledSize = NSSize(width: templateCanvas * 2, height: templateCanvas * 2)
let supersampled = NSImage(size: supersampledSize)
supersampled.lockFocus()
sourceImage.draw(in: NSRect(origin: .zero, size: supersampledSize),
                 from: NSRect(origin: .zero, size: sourceImage.size),
                 operation: .copy, fraction: 1)
supersampled.unlockFocus()

// MARK: - 3. 重映射到 Apple 模板：把原 928pt 绘制空间放入 824pt 居中框
// 所有坐标 × 2（因为 supersampled 是 2048px 对应 1024pt）
let scaleFactor: CGFloat = 2.0
let finalCanvasSize = NSSize(width: templateCanvas * scaleFactor, height: templateCanvas * scaleFactor)
let finalImage = NSImage(size: finalCanvasSize)
finalImage.lockFocus()

let destRect = NSRect(
    x: drawOrigin * scaleFactor,
    y: drawOrigin * scaleFactor,
    width: drawSize * scaleFactor,
    height: drawSize * scaleFactor
)
supersampled.draw(in: destRect,
                  from: NSRect(origin: .zero, size: supersampledSize),
                  operation: .copy, fraction: 1)
finalImage.unlockFocus()

// MARK: - 4. 输出 PNG
guard
    let tiff = finalImage.tiffRepresentation,
    let bitmap = NSBitmapImageRep(data: tiff),
    let data = bitmap.representation(using: .png, properties: [:])
else {
    fputs("Failed to encode PNG\n", stderr)
    exit(1)
}

try data.write(to: URL(fileURLWithPath: outputPath))
print("Wrote \(outputPath) (\(data.count) bytes)")
