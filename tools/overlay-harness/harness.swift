// Overlay test harness: synthetic input + a click-target window placed under a pet.
// Needs Accessibility (to post events) for the process that runs it.
//
//   harness target X Y W H SECONDS   normal-level window; prints "TARGET CLICK x,y" on mouse down
//   harness move X Y                 move the cursor (global points, top-left origin)
//   harness click X Y [opt]          left click, optionally with ⌥ flag
//   harness opt down|up              press or release the ⌥ key
import AppKit

let args = CommandLine.arguments
let src = CGEventSource(stateID: .hidSystemState)

func post(_ e: CGEvent?) { e?.post(tap: .cghidEventTap) }

func point(_ x: String, _ y: String) -> CGPoint { CGPoint(x: Double(x)!, y: Double(y)!) }

final class ClickView: NSView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {
        let p = NSEvent.mouseLocation
        print("TARGET CLICK \(Int(p.x)),\(Int(p.y))")
        fflush(stdout)
    }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.systemTeal.withAlphaComponent(0.35).setFill()
        dirtyRect.fill()
    }
}

switch args.count > 1 ? args[1] : "" {
case "target":
    let (x, y, w, h) = (Double(args[2])!, Double(args[3])!, Double(args[4])!, Double(args[5])!)
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    // Convert top-left global coords to AppKit's bottom-left coords on the main screen.
    let screenH = NSScreen.screens[0].frame.height
    let win = NSWindow(contentRect: NSRect(x: x, y: screenH - y - h, width: w, height: h),
                       styleMask: [.borderless], backing: .buffered, defer: false)
    win.contentView = ClickView()
    win.level = .normal
    win.orderFrontRegardless()
    print("TARGET READY"); fflush(stdout)
    DispatchQueue.main.asyncAfter(deadline: .now() + Double(args[6])!) { exit(0) }
    app.run()
case "move":
    post(CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: point(args[2], args[3]), mouseButton: .left))
case "click":
    let p = point(args[2], args[3])
    let flags: CGEventFlags = args.count > 4 && args[4] == "opt" ? .maskAlternate : []
    for type in [CGEventType.leftMouseDown, .leftMouseUp] {
        let e = CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p, mouseButton: .left)
        e?.flags = flags
        post(e)
        usleep(30_000)
    }
case "opt":
    let down = args[2] == "down"
    let e = CGEvent(keyboardEventSource: src, virtualKey: 58, keyDown: down) // 58 = left ⌥
    e?.flags = down ? .maskAlternate : []
    post(e)
default:
    FileHandle.standardError.write("usage: harness target|move|click|opt …\n".data(using: .utf8)!)
    exit(2)
}
