import Foundation
import AppKit

let repoRoot = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let svgURL = repoRoot.appendingPathComponent("design/icon/VacuaIcon.svg")
let appiconsetDir = repoRoot.appendingPathComponent("apps/macos/Vacua/Resources/Assets.xcassets/AppIcon.appiconset")
let assetsDir = repoRoot.appendingPathComponent("apps/macos/Vacua/Resources/Assets.xcassets")

try FileManager.default.createDirectory(at: appiconsetDir, withIntermediateDirectories: true)

// 1. Write root Assets.xcassets Contents.json
let rootContents = """
{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
"""
try rootContents.write(to: assetsDir.appendingPathComponent("Contents.json"), atomically: true, encoding: .utf8)

// 2. Load SVG
guard let image = NSImage(contentsOf: svgURL) else {
    fatalError("Failed to load SVG from \(svgURL.path)")
}

struct IconSpec {
    let size: Int
    let scale: Int
    let idiom: String
    
    var pixelSize: Int { size * scale }
    var filename: String { "icon_\(size)x\(size)@\(scale)x.png" }
}

let specs: [IconSpec] = [
    IconSpec(size: 16, scale: 1, idiom: "mac"),
    IconSpec(size: 16, scale: 2, idiom: "mac"),
    IconSpec(size: 32, scale: 1, idiom: "mac"),
    IconSpec(size: 32, scale: 2, idiom: "mac"),
    IconSpec(size: 128, scale: 1, idiom: "mac"),
    IconSpec(size: 128, scale: 2, idiom: "mac"),
    IconSpec(size: 256, scale: 1, idiom: "mac"),
    IconSpec(size: 256, scale: 2, idiom: "mac"),
    IconSpec(size: 512, scale: 1, idiom: "mac"),
    IconSpec(size: 512, scale: 2, idiom: "mac")
]

for spec in specs {
    let px = CGFloat(spec.pixelSize)
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: spec.pixelSize,
        pixelsHigh: spec.pixelSize,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    )!
    
    NSGraphicsContext.saveGraphicsState()
    let context = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current = context
    context?.imageInterpolation = .high
    
    // Draw SVG scaled to target pixel dimensions
    image.draw(in: NSRect(x: 0, y: 0, width: px, height: px),
               from: .zero,
               operation: .copy,
               fraction: 1.0)
    
    NSGraphicsContext.restoreGraphicsState()
    
    guard let pngData = rep.representation(using: .png, properties: [:]) else {
        fatalError("Failed to encode PNG for \(spec.filename)")
    }
    
    let destURL = appiconsetDir.appendingPathComponent(spec.filename)
    try pngData.write(to: destURL)
    print("Generated: \(spec.filename) (\(spec.pixelSize)x\(spec.pixelSize))")
}

// 3. Write AppIcon.appiconset/Contents.json
var imagesJson: [[String: String]] = []
for spec in specs {
    imagesJson.append([
        "size": "\(spec.size)x\(spec.size)",
        "idiom": spec.idiom,
        "filename": spec.filename,
        "scale": "\(spec.scale)x"
    ])
}

let appIconContents: [String: Any] = [
    "images": imagesJson,
    "info": [
        "author": "xcode",
        "version": 1
    ]
]

let jsonData = try JSONSerialization.data(withJSONObject: appIconContents, options: [.prettyPrinted, .sortedKeys])
try jsonData.write(to: appiconsetDir.appendingPathComponent("Contents.json"))
print("Generated AppIcon Contents.json successfully.")
