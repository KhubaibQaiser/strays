// Gate 0 native overlay prototype: Rive Apple runtime (Metal) in transparent always-on-top panels.
// Clicks pass through unless ⌥ is held while the cursor is over a pet.
//
// STRAYS_MODE=A (default): one small panel per pet.  STRAYS_MODE=B: one panel covering the main screen.
// STRAYS_FPS (default 30): preferred frame rate.  STRAYS_DRAW=always|changed (default changed).
import AppKit
import RiveRuntime

let env = ProcessInfo.processInfo.environment
let modeB = env["STRAYS_MODE"] == "B"
let fps = Int(env["STRAYS_FPS"] ?? "30") ?? 30
let drawOnChanged = env["STRAYS_DRAW"] != "always"
let petCount = Int(env["STRAYS_PETS"] ?? "3") ?? 3
let petSize: CGFloat = 160

func log(_ s: String) { print(s); fflush(stdout) }

// coatBase, coatShade, coatHighlight, outline as 0xRRGGBB
let looks: [[UInt32]] = [
    [0xEFE1CC, 0xD9C4A6, 0xFFF8EE, 0x7A5C46],
    [0x34343A, 0x26262B, 0x55555E, 0xA9A9B6],
    [0xE8A15C, 0xCB8240, 0xFBE3C6, 0x7A4520],
]
let colourNames = ["coatBase", "coatShade", "coatHighlight", "outline"]

final class OverlayPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Catches ⌥-clicks for the pet views it contains; only receives events while the panel is interactive.
final class PetContainer: NSView {
    var petViews: [(index: Int, view: NSView)] = []
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {
        let p = convert(event.locationInWindow, from: nil)
        if event.modifierFlags.contains(.option),
           let hit = petViews.first(where: { $0.view.frame.contains(p) }) {
            log("CLICK pet=\(hit.index)")
        }
    }
}

final class App: NSObject, NSApplicationDelegate {
    var panels: [OverlayPanel] = []
    var containers: [PetContainer] = []
    var riveModels: [RiveViewModel] = []
    var petFrames: [CGRect] = [] // global AppKit coords (bottom-left origin)
    var petViews: [NSView] = []
    var hovered: Int?
    var timer: Timer?

    func applicationDidFinishLaunching(_ note: Notification) {
        NSApp.setActivationPolicy(.accessory)
        let screen = NSScreen.main!
        let area = screen.visibleFrame
        petFrames = (0..<petCount).map { i in
            CGRect(x: area.maxX - (petSize + 40) * CGFloat(i + 1), y: area.minY, width: petSize, height: petSize)
        }

        if modeB {
            let panel = makePanel(frame: screen.frame)
            for (i, f) in petFrames.enumerated() {
                let local = f.offsetBy(dx: -screen.frame.minX, dy: -screen.frame.minY)
                addPet(i, frame: local, to: panel)
            }
        } else {
            for (i, f) in petFrames.enumerated() {
                let panel = makePanel(frame: f)
                addPet(i, frame: CGRect(origin: .zero, size: f.size), to: panel)
            }
        }
        panels.forEach { $0.orderFrontRegardless() }

        let screenH = NSScreen.screens[0].frame.height
        let rects = petFrames.map { "\(Int($0.minX)),\(Int(screenH - $0.maxY)),\(Int($0.width)),\(Int($0.height))" }
        log("READY mode=\(modeB ? "B" : "A") fps=\(fps) draw=\(drawOnChanged ? "changed" : "always") rects=\(rects.joined(separator: ";"))")
        schedulePoll(fast: false)
    }

    func makePanel(frame: CGRect) -> OverlayPanel {
        let p = OverlayPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = false
        p.level = .floating
        p.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary, .ignoresCycle]
        p.ignoresMouseEvents = true
        // keep pets out of screen sharing and recordings (tests opt back in to take screenshots)
        p.sharingType = env["STRAYS_CAPTURABLE"] == "1" ? .readOnly : .none
        let container = PetContainer(frame: CGRect(origin: .zero, size: frame.size))
        p.contentView = container
        panels.append(p)
        containers.append(container)
        return p
    }

    func addPet(_ i: Int, frame: CGRect, to panel: OverlayPanel) {
        guard let model = try? RiveModel(fileName: "cat", in: .module, loadCdn: false) else {
            log("LOADERROR pet=\(i)"); return
        }
        var bound = 0
        model.enableAutoBind { instance in
            for (n, name) in colourNames.enumerated() {
                guard let prop = instance.colorProperty(fromPath: name) else { continue }
                let c = looks[i % looks.count][n]
                prop.value = NSColor(srgbRed: CGFloat(c >> 16 & 0xFF) / 255, green: CGFloat(c >> 8 & 0xFF) / 255,
                                     blue: CGFloat(c & 0xFF) / 255, alpha: 1)
                bound += 1
            }
        }
        let vm = RiveViewModel(model, stateMachineName: "Idle")
        if #available(macOS 14, *) {
            vm.setPreferredFramesPerSecond(preferredFramesPerSecond: fps)
        } // ponytail: on macOS 13 the runtime draws at display rate; revisit if 13 stays supported
        let view = vm.createRiveView()
        view.drawOptimization = drawOnChanged ? .drawOnChanged : .alwaysDraw
        view.offscreenBehavior = .playAndNoDraw
        view.frame = frame
        view.wantsLayer = true
        let container = panel.contentView as! PetContainer
        container.addSubview(view)
        if #available(macOS 14, *) { // the view's display link exists only once it is in a window
            view.setPreferredFramesPerSecond(preferredFramesPerSecond: fps)
        }
        if env["STRAYS_PAUSED"] == "1" { vm.pause() }
        // Burst test: hold still, play for STRAYS_BURST seconds every 8 s (staggered per pet).
        if let burst = Double(env["STRAYS_BURST"] ?? "") {
            vm.pause()
            Timer.scheduledTimer(withTimeInterval: 8, repeats: true) { _ in
                DispatchQueue.main.asyncAfter(deadline: .now() + Double(i) * 2.5) {
                    vm.play()
                    DispatchQueue.main.asyncAfter(deadline: .now() + burst) { vm.pause() }
                }
            }
        }
        container.petViews.append((i, view))
        riveModels.append(vm)
        petViews.append(view)
        log("LOADED pet=\(i) colours=\(bound)/4")
    }

    func schedulePoll(fast: Bool) {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: fast ? 1.0 / 30 : 0.25, repeats: true) { [weak self] _ in self?.poll() }
        timer?.tolerance = fast ? 0.005 : 0.05
    }

    func poll() {
        // Reading cursor position and modifier state needs no Accessibility/Input Monitoring permission.
        let p = NSEvent.mouseLocation
        let held = NSEvent.modifierFlags.contains(.option)
        let over = petFrames.firstIndex { hitTest($0, p) }
        let target = held ? over : nil

        if target != hovered {
            if let h = hovered { setGlow(petViews[h], on: false) }
            if let t = target { setGlow(petViews[t], on: true) }
            log("HOVER \(target.map { "Some(\($0))" } ?? "None")")
            hovered = target
        }
        for (wi, panel) in panels.enumerated() {
            let want = modeB ? target != nil : target == wi
            if panel.ignoresMouseEvents == want { panel.ignoresMouseEvents = !want }
        }
        let near = petFrames.contains { $0.insetBy(dx: -150, dy: -150).contains(p) }
        if near != (timer!.timeInterval < 0.1) { schedulePoll(fast: near) }
    }

    // Body ellipse inside the pet square, not its transparent margin.
    func hitTest(_ f: CGRect, _ p: CGPoint) -> Bool {
        let cx = f.midX, cy = f.minY + f.height * 0.4
        let dx = (p.x - cx) / (f.width * 0.38), dy = (p.y - cy) / (f.height * 0.38)
        return dx * dx + dy * dy <= 1
    }

    func setGlow(_ v: NSView, on: Bool) {
        v.layer?.shadowColor = NSColor(calibratedRed: 0.47, green: 0.67, blue: 1, alpha: 1).cgColor
        v.layer?.shadowRadius = 6
        v.layer?.shadowOffset = .zero
        v.layer?.shadowOpacity = on ? 0.9 : 0
    }
}

let app = NSApplication.shared
let delegate = App()
app.delegate = delegate
app.run()
