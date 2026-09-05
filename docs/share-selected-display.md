# Share a selected Mac display over VNC

Vinny lets each server configuration capture one connected display. You can give separate displays their own ports, capture settings, and remote-control policy.

First complete the [TigerVNC connection guide](first-connection.md). The steps below keep the same SSH-tunnel setup.

## Choose what to share

1. Open Vinny from its menu-bar icon.
2. In the server card, open **Display** and select the monitor you want to share. Labels include the display name.
3. Set **Maximum width** and **Frame rate** for the capture. Vinny accepts widths from 320 to 7680 and frame rates from 1 to 60. These are settings, not guaranteed delivered performance. Start with the defaults of 1920 and 20.
4. Choose **Apply & restart**, then reconnect your viewer.
5. Move a window on the selected display and confirm you see the same movement remotely.

Maximum width changes the captured image size. It does not change the Mac's display resolution or create a virtual display. Reducing width or frame rate can reduce the amount of image data you ask Vinny to capture and send.

## Give another display its own connection

Add a server in Vinny, choose the other display, and note its port. New configurations use the next unused configured port starting at `5900`, but another application may already be using that port.

Keep **Listen on** at `127.0.0.1`. New servers start disabled: turn **Enabled** on and choose **Apply & restart**. For a second server on `5901`, open a second tunnel on the viewing computer:

```bash
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:15901:127.0.0.1:5901 YOUR_USER@YOUR_MAC
```

Connect TigerVNC Viewer to `127.0.0.1::15901`. Keep the terminal open. See the [first connection guide](first-connection.md) for SSH setup and the local-access limitations of this recipe.

These listeners share displays from the same Mac session. They do not create isolated user desktops. Remote input affects the shared Mac.

## Make a listener view-only

Turn off **Allow keyboard and mouse** and choose **Apply & restart**. After reconnecting, check that remote clicks and typing no longer affect the Mac.

This also blocks incoming clipboard changes. Viewers can still receive screen and outgoing clipboard contents, so view-only is an input restriction rather than a privacy boundary.

## When a display changes

If you unplug a monitor or change the display arrangement, reopen Vinny and confirm the selected display before reconnecting. A disconnected selection must be replaced with a connected display. Changing a server's configuration restarts that listener and disconnects its clients.
