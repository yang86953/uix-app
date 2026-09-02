#!/bin/zsh
# 通过 KWin Scripting 读取 uix-app 窗口的 compositor 侧几何
Q=/usr/sbin/qdbus6
ID=$($Q org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript /home/yang/data/code/uix-app/shadow-scratch/win-info.js)
sleep 0.3
$Q org.kde.KWin /Scripting/Script$ID org.kde.kwin.Script.run
sleep 0.6
$Q org.kde.KWin /Scripting/Script$ID org.kde.kwin.Script.stop >/dev/null 2>&1
$Q org.kde.KWin /Scripting/Script$ID org.kde.kwin.Script.remove 2>/dev/null
