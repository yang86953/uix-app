const targets = workspace.windowList().filter(w => w.resourceClass == "uix-app");
print("uix windows: " + targets.length);
for (const w of targets) {
    print("window: " + w.caption + " minimized=" + w.minimized);
    w.minimized = false;
}
