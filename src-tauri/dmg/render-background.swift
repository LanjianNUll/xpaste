import AppKit
import CoreGraphics

let W: CGFloat = 660
let H: CGFloat = 420

// 图标槽位（Finder 坐标系，原点在窗口内容区左上角）
let appCenter = CGPoint(x: 165, y: 216)
let appsCenter = CGPoint(x: 495, y: 216)

struct Theme {
    let name: String
    let top: UInt32
    let bottom: UInt32
    let glow: UInt32
    let glowAlpha: CGFloat
    let title: UInt32
    let hint: UInt32
    let subtle: UInt32
    let haloAlpha: CGFloat
    let arrowFrom: UInt32
    let arrowTo: UInt32
    let accent: UInt32
    let accentAlpha: CGFloat
}

let lightTheme = Theme(
    name: "light",
    top: 0xFCFDFF, bottom: 0xE5ECF8,
    glow: 0xFFFFFF, glowAlpha: 0.75,
    title: 0x1E2733, hint: 0x77839A, subtle: 0xA7B1C1,
    haloAlpha: 0.55,
    arrowFrom: 0x74C3F0, arrowTo: 0x3E8FD6,
    accent: 0xF0A98C, accentAlpha: 0.85
)

let darkTheme = Theme(
    name: "dark",
    top: 0x212C40, bottom: 0x0E1420,
    glow: 0x4A7FBF, glowAlpha: 0.38,
    title: 0xF3F6FB, hint: 0x95A3B9, subtle: 0x65718A,
    haloAlpha: 0.07,
    arrowFrom: 0x86CDF6, arrowTo: 0x4B9CE2,
    accent: 0xF2AE92, accentAlpha: 0.9
)

let mistTheme = Theme(
    name: "mist",
    top: 0xF2F6FD, bottom: 0xCDDAEE,
    glow: 0xFFFFFF, glowAlpha: 0.85,
    title: 0x1A2331, hint: 0x616E82, subtle: 0x939FB2,
    haloAlpha: 0.30,
    arrowFrom: 0x6FBDEE, arrowTo: 0x3585CF,
    accent: 0xF0A98C, accentAlpha: 0.85
)

func rgb(_ hex: UInt32) -> CGColor {
    return CGColor(
        red: CGFloat((hex >> 16) & 0xFF) / 255.0,
        green: CGFloat((hex >> 8) & 0xFF) / 255.0,
        blue: CGFloat(hex & 0xFF) / 255.0,
        alpha: 1.0
    )
}

func rgba(_ hex: UInt32, _ a: CGFloat) -> CGColor {
    return CGColor(
        red: CGFloat((hex >> 16) & 0xFF) / 255.0,
        green: CGFloat((hex >> 8) & 0xFF) / 255.0,
        blue: CGFloat(hex & 0xFF) / 255.0,
        alpha: a
    )
}

func font(_ size: CGFloat, _ weight: String) -> NSFont {
    let names = ["PingFangSC-\(weight)", "HelveticaNeue-\(weight)"]
    for n in names {
        if let f = NSFont(name: n, size: size) { return f }
    }
    return NSFont.systemFont(ofSize: size)
}

func drawText(_ text: String, centerX: CGFloat, top: CGFloat, size: CGFloat,
              weight: String, color: UInt32, tracking: CGFloat = 0) {
    var attrs: [NSAttributedString.Key: Any] = [
        .font: font(size, weight),
        .foregroundColor: NSColor(cgColor: rgb(color))!,
        .kern: tracking
    ]
    let s = NSAttributedString(string: text, attributes: attrs)
    let w = s.size().width
    // 转换为 AppKit 的底部原点坐标
    let y = H - top - s.size().height
    s.draw(at: NSPoint(x: centerX - w / 2, y: y))
    attrs.removeAll()
}

func drawTextRight(_ text: String, right: CGFloat, top: CGFloat, size: CGFloat,
                   weight: String, color: UInt32) {
    let attrs: [NSAttributedString.Key: Any] = [
        .font: font(size, weight),
        .foregroundColor: NSColor(cgColor: rgb(color))!
    ]
    let s = NSAttributedString(string: text, attributes: attrs)
    let w = s.size().width
    s.draw(at: NSPoint(x: right - w, y: H - top - s.size().height))
}

func render(_ t: Theme, scale: CGFloat, out: String) {
    let px = Int(W * scale)
    let py = Int(H * scale)
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: py,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ) else { fatalError("bitmap") }
    rep.size = NSSize(width: W, height: H)

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let ctx = NSGraphicsContext.current!.cgContext
    ctx.setAllowsAntialiasing(true)
    ctx.setShouldAntialias(true)
    ctx.interpolationQuality = .high

    // 底色渐变
    let space = CGColorSpaceCreateDeviceRGB()
    let grad = CGGradient(colorsSpace: space,
                          colors: [rgb(t.top), rgb(t.bottom)] as CFArray,
                          locations: [0, 1])!
    ctx.drawLinearGradient(grad,
                           start: CGPoint(x: 0, y: H),
                           end: CGPoint(x: 0, y: 0),
                           options: [])

    // 顶部柔光
    let glowGrad = CGGradient(colorsSpace: space,
                              colors: [rgba(t.glow, t.glowAlpha), rgba(t.glow, 0)] as CFArray,
                              locations: [0, 1])!
    let glowCenter = CGPoint(x: W / 2, y: H - 30)
    ctx.drawRadialGradient(glowGrad, startCenter: glowCenter, startRadius: 0,
                           endCenter: glowCenter, endRadius: 320, options: [])

    // 图标槽位的柔光底，让浅色图标在背景上"落得住"
    for c in [appCenter, appsCenter] {
        let cc = CGPoint(x: c.x, y: H - c.y)
        let halo = CGGradient(colorsSpace: space,
                              colors: [rgba(0xFFFFFF, t.haloAlpha),
                                       rgba(0xFFFFFF, t.haloAlpha * 0.45),
                                       rgba(0xFFFFFF, t.haloAlpha * 0.12),
                                       rgba(0xFFFFFF, 0)] as CFArray,
                              locations: [0, 0.35, 0.68, 1])!
        ctx.drawRadialGradient(halo, startCenter: cc, startRadius: 0,
                               endCenter: cc, endRadius: 105, options: [])
    }

    // 轻微暗角，避免大面积底色显得平
    let vig = CGGradient(colorsSpace: space,
                         colors: [rgba(0x0A1524, 0), rgba(0x0A1524, 0.055)] as CFArray,
                         locations: [0.55, 1])!
    let center = CGPoint(x: W / 2, y: H / 2)
    ctx.drawRadialGradient(vig, startCenter: center, startRadius: 60,
                           endCenter: center, endRadius: 430, options: [.drawsAfterEndLocation])

    // 拖拽箭头
    let ay = H - appCenter.y
    let shaftStart = CGPoint(x: 250, y: ay)
    let shaftEnd = CGPoint(x: 388, y: ay)
    let arrowGrad = CGGradient(colorsSpace: space,
                               colors: [rgb(t.arrowFrom), rgb(t.arrowTo)] as CFArray,
                               locations: [0, 1])!
    ctx.saveGState()
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    ctx.setLineWidth(7.5)
    ctx.move(to: shaftStart)
    ctx.addLine(to: shaftEnd)
    ctx.replacePathWithStrokedPath()
    ctx.clip()
    ctx.drawLinearGradient(arrowGrad, start: CGPoint(x: 250, y: ay),
                           end: CGPoint(x: 402, y: ay), options: [])
    ctx.restoreGState()

    // 箭头头部
    ctx.saveGState()
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    ctx.setLineWidth(7.5)
    ctx.move(to: CGPoint(x: 374, y: ay + 19))
    ctx.addLine(to: CGPoint(x: 400, y: ay))
    ctx.addLine(to: CGPoint(x: 374, y: ay - 19))
    ctx.replacePathWithStrokedPath()
    ctx.clip()
    ctx.drawLinearGradient(arrowGrad, start: CGPoint(x: 250, y: ay),
                           end: CGPoint(x: 402, y: ay), options: [])
    ctx.restoreGState()

    // 顶部标题
    drawText("Xpaste", centerX: W / 2, top: 46, size: 27, weight: "Semibold",
             color: t.title, tracking: 0.4)

    // 底部提示
    drawText("将 Xpaste 拖入 Applications 文件夹，即可完成安装",
             centerX: W / 2, top: 338, size: 12.5, weight: "Regular",
             color: t.hint, tracking: 0.2)

    // 右下角版本号
    drawTextRight("v2.0.14", right: 620, top: 370, size: 10.5,
                  weight: "Regular", color: t.subtle)

    NSGraphicsContext.restoreGraphicsState()

    guard let data = rep.representation(using: .png, properties: [:]) else {
        fatalError("png")
    }
    try! data.write(to: URL(fileURLWithPath: out))
    print("wrote \(out) \(px)x\(py)")
}

let args = CommandLine.arguments
let themeName = args.count > 1 ? args[1] : "light"
let outDir = args.count > 2 ? args[2] : "/tmp/dmg_bg"
let t: Theme
switch themeName {
case "dark": t = darkTheme
case "mist": t = mistTheme
default: t = lightTheme
}

try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
render(t, scale: 1, out: "\(outDir)/bg.png")
render(t, scale: 2, out: "\(outDir)/bg@2x.png")
