// Overlay test harness: synthetic input + a click-target window placed under a pet.
// Needs Accessibility (to post events) for the process that runs it.
//
//   harness target X Y W H SECONDS   normal-level window; prints "TARGET CLICK x,y" on mouse down
//   harness move X Y                 move the cursor (global points, top-left origin)
//   harness click X Y [opt]          left click, optionally with ⌥ flag
//   harness drag X1 Y1 X2 Y2         ⌥-drag from one point to the other
//   harness screen                   print the main display's width and height in points
//   harness opt down|up              press or release the ⌥ key
//   harness esc                      press Escape (closes an open menu)
import AppKit

let args = CommandLine.arguments
let src = CGEventSource(stateID: .hidSystemState)

// The short wait lets the event reach the HID system before this process exits; without it ~1 in 4 ⌥ events was lost.
func post(_ e: CGEvent?) { e?.post(tap: .cghidEventTap); usleep(50_000) }

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
    let flags: CGEventFlags = args.count > 4 && args[4] == "opt" ? .maskAlternate
        : args.count > 4 && args[4] == "optshift" ? [.maskAlternate, .maskShift] : []
    for type in [CGEventType.leftMouseDown, .leftMouseUp] {
        let e = CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p, mouseButton: .left)
        e?.flags = flags
        post(e)
        usleep(30_000)
    }
case "screen": // the main display's size in points
    let f = NSScreen.main?.frame ?? .zero
    print(Int(f.width), Int(f.height))
case "drag": // drag x1 y1 x2 y2: ⌥-press at the first point, move in steps, let go at the second
    let (a, b) = (point(args[2], args[3]), point(args[4], args[5]))
    func at(_ t: Double) -> CGPoint { CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t) }
    let steps = 20
    var e = CGEvent(mouseEventSource: src, mouseType: .leftMouseDown, mouseCursorPosition: a, mouseButton: .left)
    e?.flags = .maskAlternate
    post(e)
    for i in 1...steps {
        e = CGEvent(mouseEventSource: src, mouseType: .leftMouseDragged, mouseCursorPosition: at(Double(i) / Double(steps)), mouseButton: .left)
        e?.flags = .maskAlternate
        e?.post(tap: .cghidEventTap)
        usleep(25_000)
    }
    usleep(200_000)
    e = CGEvent(mouseEventSource: src, mouseType: .leftMouseUp, mouseCursorPosition: b, mouseButton: .left)
    e?.flags = .maskAlternate
    post(e)
case "opt":
    let down = args[2] == "down"
    let e = CGEvent(keyboardEventSource: src, virtualKey: 58, keyDown: down) // 58 = left ⌥
    e?.type = .flagsChanged // a modifier is a flags change, not a key press; keyDown/keyUp left HID state stale
    e?.flags = down ? .maskAlternate : []
    post(e)
case "esc":
    for down in [true, false] { post(CGEvent(keyboardEventSource: src, virtualKey: 53, keyDown: down)) }
default:
    FileHandle.standardError.write("usage: harness target|move|click|drag|screen|opt|esc …\n".data(using: .utf8)!)
    exit(2)
}
