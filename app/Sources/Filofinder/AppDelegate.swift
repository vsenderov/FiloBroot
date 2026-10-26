import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var windows: [FileManagerWindowController] = []

    func applicationWillFinishLaunching(_ notification: Notification) {
        // Before launch finishes, so a reveal that launched the app is handled.
        DefaultBrowser.installRevealHandler { [weak self] paths in self?.reveal(paths) }
    }

    private var launched = false
    private var pendingReveal: [String]?
    private var pendingOpen: [URL] = []

    /// The window that handles requests from other apps (opens one if needed).
    private func frontController() -> FileManagerWindowController {
        if let key = windows.first(where: { $0.window?.isKeyWindow == true }) { return key }
        if let visible = windows.first(where: { $0.window?.isVisible == true }) { return visible }
        openWindow(path: nil)
        return windows.last!
    }

    /// The 'mvis' reveal event (NSWorkspace activateFileViewerSelectingURLs,
    /// i.e. "Reveal in Finder" in Cursor/VS Code/Electron apps when this app
    /// is the NSFileViewer): the enclosing folder, items selected.
    private func reveal(_ paths: [String]) {
        guard launched else { pendingReveal = (pendingReveal ?? []) + paths; return }
        let controller = frontController()
        controller.reveal(paths)
        controller.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    /// Open Documents ('odoc'): `open -a FiloBroot <path>`, dropping onto the
    /// Dock icon, "Open With", or a folder handed to the default viewer.
    /// Folders open in a new tab; files (and packages such as .app) open
    /// their folder with the file selected.
    func application(_ application: NSApplication, open urls: [URL]) {
        guard launched else { pendingOpen += urls; return }
        let controller = frontController()
        var files: [String] = []
        for url in urls where url.isFileURL {
            let path = url.path
            var isDir: ObjCBool = false
            guard FileManager.default.fileExists(atPath: path, isDirectory: &isDir) else { continue }
            if isDir.boolValue && !NSWorkspace.shared.isFilePackage(atPath: path) {
                controller.openFolderInNewTab(path)
            } else {
                files.append(path)
            }
        }
        if !files.isEmpty { controller.reveal(files) }
        controller.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc func toggleDefaultFileBrowser(_ sender: Any?) {
        let on = DefaultBrowser.isDefault
        let alert = NSAlert()
        if on {
            alert.messageText = "FiloBroot is your default file browser"
            alert.informativeText = "\"Show in Finder\" in other apps opens FiloBroot. Switch back to Finder?"
            alert.addButton(withTitle: "Use Finder")
        } else {
            alert.messageText = "Make FiloBroot the default file browser?"
            alert.informativeText = "\"Show in Finder\" and \"Show in folder\" in other apps (browser downloads, editors…) will open FiloBroot instead. Finder, the Desktop and the system Open/Save windows stay as they are.\n\nOther apps pick this up after they're restarted. You can switch back here at any time."
            alert.addButton(withTitle: "Make Default")
        }
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        DefaultBrowser.set(!on)
        UI.hud(on ? "Finder is the default file browser again" : "FiloBroot is now the default file browser",
               in: NSApp.keyWindow)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.build()
        // A folder passed on the command line opens in the first tab;
        // otherwise start at the empty state, like captools.
        var start = CommandLine.arguments.dropFirst().first { !$0.hasPrefix("-") }
        // Launched to open a folder: start there instead of adding a tab.
        if start == nil, let first = pendingOpen.first, first.isFileURL, isPlainFolder(first.path) {
            start = first.path
            pendingOpen.removeFirst()
        }
        openWindow(path: start)
        if #available(macOS 14.0, *) {
            NSApp.activate()
        } else {
            NSApp.activate(ignoringOtherApps: true)
        }
        askForDiskAccessIfNeeded()
        launched = true
        if let paths = pendingReveal { pendingReveal = nil; reveal(paths) }
        if !pendingOpen.isEmpty {
            let urls = pendingOpen
            pendingOpen = []
            application(NSApp, open: urls)
        }
    }

    private func isPlainFolder(_ path: String) -> Bool {
        var isDir: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDir) && isDir.boolValue
            && !NSWorkspace.shared.isFilePackage(atPath: path)
    }

    private var accessSheet: DiskAccessSheet?

    /// Filofinder browses the whole disk, so ask for Full Disk Access up front.
    private func askForDiskAccessIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        guard !DiskAccess.isGranted, !DiskAccess.dontAskAgain, env["FILOFINDER_SNAPSHOT"] == nil,
              let main = windows.first, let window = main.window else { return }
        let sheet = DiskAccessSheet()
        sheet.onGranted = { [weak main] in
            UI.hud("Full Disk Access is on — every folder is available", in: main?.window)
            main?.refresh(nil)
        }
        accessSheet = sheet
        DispatchQueue.main.async { sheet.present(on: window) }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    /// Clicking the Dock icon with no visible window opens one (Finder behaviour).
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { openWindow(path: nil) }
        return true
    }

    @objc func newWindow(_ sender: Any?) {
        openWindow(path: nil)
    }

    private func openWindow(path: String?) {
        let controller = FileManagerWindowController(path: path)
        windows.append(controller)
        controller.showWindow(nil)
        controller.window?.makeKeyAndOrderFront(nil)
        #if DEBUG
        installTestHooks(controller)
        #endif
    }

    #if DEBUG
    /// Development-only hooks used for automated checks (not in release builds).
    private func installTestHooks(_ controller: FileManagerWindowController) {
        // FILOFINDER_SPLIT=1 opens straight into split view.
        let env = ProcessInfo.processInfo.environment
        if env["FILOFINDER_SPLIT"] == "1" { controller.toggleSplit(nil) }
        if env["FILOFINDER_FOCUS"] == "right" {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { controller.focus(.right) }
        }
        // Test hook: FILOFINDER_TREE_CLICK=<row> clicks a sidebar row.
        if let row = env["FILOFINDER_TREE_CLICK"].flatMap(Int.init) {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { controller.debugTreeClick(row: row) }
            DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) {
                FileHandle.standardError.write(Data("[tree] now showing \(controller.activeTab.path)\n".utf8))
            }
        }
        // FILOFINDER_TOGGLE_TREE=N toggles the folder tree N times.
        if let n = env["FILOFINDER_TOGGLE_TREE"].flatMap(Int.init) {
            for i in 0..<n {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5 + Double(i) * 0.4) { controller.toggleTree(nil) }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                FileHandle.standardError.write(Data(controller.debugDumpLayout().utf8))
            }
        }
        // FILOFINDER_FLIP_APPEARANCE=dark|light switches appearance while running.
        if let mode = env["FILOFINDER_FLIP_APPEARANCE"] {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                NSApp.appearance = NSAppearance(named: mode == "dark" ? .darkAqua : .aqua)
            }
        }
        if env["FILOFINDER_SELECT_ALL"] == "1" {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { controller.selectAllItems(nil) }
        }
        // Test hook: FILOFINDER_SNAPSHOT=/path.png renders this window to a
        // PNG (no screen-recording permission needed), then quits.
        if let out = env["FILOFINDER_SNAPSHOT"] {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.8) {
                if let view = controller.window?.contentView?.superview ?? controller.window?.contentView,
                   let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
                    view.cacheDisplay(in: view.bounds, to: rep)
                    try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: out))
                }
                NSApp.terminate(nil)
            }
        }
    }
    #endif
}
