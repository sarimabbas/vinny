# Connect to your Mac with TigerVNC and Vinny

This guide uses an SSH tunnel to reach Vinny's default loopback listener. SSH protects traffic between the computers without exposing a VNC port to your network.

## Before you start

- Install [Vinny](../README.md#install) on the Mac whose display you want to share.
- Install [TigerVNC Viewer](https://tigervnc.org/) on the viewing computer.
- Make sure the viewing computer can reach the Mac over your local network or an existing private network. This guide does not configure internet routing.
- Have a Mac account authorised for SSH access and an SSH client on the viewing computer.

## 1. Open Vinny and grant access

Open Vinny from Applications. Grant **Screen Recording** and **Accessibility** when prompted. If you dismissed a prompt, find Vinny in the corresponding Privacy & Security settings. Older macOS releases call these Security & Privacy preferences. Reopen Vinny if macOS asks you to.

Both permissions are required to start a server, including a view-only server. Accessibility allows Vinny to forward remote input.

Keep the initial settings:

| Setting | Value |
| --- | --- |
| Enabled | On |
| Display | The display you want to share |
| Listen on | `127.0.0.1` |
| Port | `5900` |
| Encrypted + password | Off for this SSH-tunnel recipe |
| Allow keyboard and mouse | On, or off for view-only access |

If you change settings, choose **Apply & restart**. Confirm that Vinny reports the server as **listening**.

## 2. Enable SSH access to the Mac

In **System Settings → General → Sharing → Remote Login**, enable Remote Login and limit access to the account you intend to use. On older macOS versions, look under **System Preferences → Sharing**. Full disk access for remote users is not needed for this tunnel.

Use the account and host shown in Remote Login's SSH command. See [Apple's Remote Login instructions](https://support.apple.com/guide/mac-help/mchlp1066/mac).

## 3. Open the tunnel from the viewing computer

Replace `YOUR_USER` and `YOUR_MAC` below. Run the command on the **viewing computer**, not the Mac being shared:

```bash
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:15900:127.0.0.1:5900 YOUR_USER@YOUR_MAC
```

Verify the SSH host identity when connecting for the first time. Authenticate with your Mac account's SSH key or password. A quiet terminal that stays open is normal: `-N` opens the tunnel without a shell.

Port `15900` is a local port on the viewing computer. The tunnel forwards it to Vinny's `5900` on the Mac. Both ends bind to loopback. Keep this terminal open for the session.

The VNC listener has no local authentication in this recipe. Other local users and processes on either computer may be able to access the session through its listener or tunnel. Use trusted computers, and keep Vinny's address at `127.0.0.1`.

## 4. Connect TigerVNC

Open TigerVNC Viewer and enter:

```text
127.0.0.1::15900
```

Use **two colons**. TigerVNC treats this as an explicit port rather than a display number. If your installation provides the command-line viewer:

```bash
vncviewer 127.0.0.1::15900
```

The VNC layer uses security type **None** because SSH protects this route. If your viewer configuration disables that type, allow it for this tunnel connection only. There is no Vinny password prompt in this recipe. See the [TigerVNC viewer manual](https://tigervnc.org/doc/vncviewer.html) for connection syntax and security options.

## 5. Check the result

You should see the display selected in Vinny. Open a blank document on the shared Mac and check pointer movement and typing from the viewer.

For view-only access, turn off **Allow keyboard and mouse** in Vinny, choose **Apply & restart**, and reconnect. This blocks keyboard, pointer, and incoming clipboard changes. It still shares the screen and outgoing clipboard contents.

To finish, disconnect the viewer and press **Control-C** in the SSH terminal. The tunnel closes, while Vinny continues listening locally. Quit Vinny or disable its server when you no longer want to share. You can also turn Remote Login back off if you enabled it only for this session.

For usage feedback, include your macOS version, viewer version, and the step where you got stuck in [a connection feedback issue](https://github.com/sarimabbas/vinny/issues/new/choose). Do not include passwords or private keys.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Vinny does not start a server | Grant both Screen Recording and Accessibility. Look for an error on the server card, then apply again. |
| Address already in use on the Mac | Built-in Screen Sharing or another VNC server may own `5900`. Choose `5901` in Vinny, apply, and change the final tunnel destination to `127.0.0.1:5901`. The viewer still connects to `127.0.0.1::15900`. |
| SSH says connection refused or times out | Confirm Remote Login is on, the Mac is awake and reachable, and the hostname or IP matches its settings. This is an SSH/network issue before VNC connects. |
| SSH says permission denied | Check the SSH username, authentication, and the list of users allowed in Remote Login. |
| SSH cannot bind local port `15900` | Close an earlier tunnel, or use `15901` as the first port in `-L` and connect the viewer to `127.0.0.1::15901`. |
| Viewer connection refused | Keep the tunnel terminal open. Check Vinny's running state and that its port matches the tunnel destination. |
| No matching security types | For this loopback tunnel recipe, the viewer must allow security type `None`. A viewer configured to require VNC-layer TLS will not match these settings. |
| Blank or wrong display | Check Screen Recording permission and the selected connected display. Reopen the app if permissions changed. Re-select the display after disconnecting a monitor. |
| Picture works, input does not | Check Accessibility and **Allow keyboard and mouse**, then apply and reconnect. |
| Session disconnects after changing settings | **Apply & restart** restarts that listener. Reconnect the viewer. |

For direct encrypted VNC and legacy authentication trade-offs, see [connection security](../README.md#connection-security).
