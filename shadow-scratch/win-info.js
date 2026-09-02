for (const w of workspace.windowList()) {
    if (w.resourceClass == "uix-app") {
        print("UIXWIN " + JSON.stringify({
            caption: w.caption,
            frame: [w.frameGeometry.x, w.frameGeometry.y, w.frameGeometry.width, w.frameGeometry.height],
            client: [w.clientGeometry.x, w.clientGeometry.y, w.clientGeometry.width, w.clientGeometry.height],
            maximized: w.maximized,
            minimized: w.minimized,
            fullscreen: w.fullscreen,
            active: w.active
        }));
    }
}
