# Launch drafts and checklist

These are drafts for Sarim to review. Nothing here has been submitted, scheduled, or published. Replace the demo placeholder only after recording the working application.

## Show HN

Title: **Show HN: Vinny, a configurable open-source VNC server for macOS**

URL: https://github.com/sarimabbas/vinny

First comment:

> I built Vinny, a menu-bar VNC server for macOS. It lets you choose the display, maximum width, frame rate, listener address and port, and whether a connection can control the Mac.
>
> Each server has its own configuration. The app uses ScreenCaptureKit for capture and Rust for the server, with a native macOS interface.
>
> The first-connection guide is in the README. Vinny starts with a loopback-only listener. For access from another machine, follow the documented secure connection setup.
>
> I'd like feedback from people who need more control over individual VNC listeners. Were you able to connect, and which setting or setup step was unclear?

Before posting, add one true sentence about the specific problem that led you to build Vinny. No founder motivation has been assumed here. Include the actual demo link once available.

## Technical Mac community post

Title: **I made Vinny, a menu-bar VNC server with per-display settings**

> I'm the developer of Vinny, an open-source VNC server for macOS.
>
> You can configure a listener for a selected display, adjust maximum width and frame rate, and turn remote control on or off. Multiple listeners can use separate settings.
>
> If those controls would help your setup, I'd appreciate a test of the installation and first-connection guide: https://github.com/sarimabbas/vinny
>
> The default listener is localhost-only. The README covers secure access from a second machine and viewer compatibility. Please don't expose the default unauthenticated listener to a network.
>
> The most useful feedback is your macOS version, viewer and viewer version, whether the first connection worked, and what you were trying to do. Please keep passwords, addresses, and screen contents out of public reports.

Choose a relevant community that permits project posts. Read its current rules and adapt the format before submitting. Avoid cross-posting the same text across unrelated communities.

## Rust community post

Title: **Building a native macOS VNC server in Rust with ScreenCaptureKit**

> Vinny is a small Apache-2.0 VNC server for macOS that I've been working on.
>
> It combines ScreenCaptureKit capture bindings, a vendored rustvncserver implementation, enigo for input, and a native macOS interface. Server configurations cover display selection, width and frame-rate limits, listener settings, and remote control.
>
> Some of the implementation details that may interest other Rust developers are Retina coordinate handling, macOS privacy permissions, and the boundary between capture, RFB encoding, and native UI.
>
> Source and build instructions: https://github.com/sarimabbas/vinny
>
> I'd welcome implementation feedback and connection reports from people trying it with their VNC viewer. The README describes the default loopback listener and the secure remote connection options.

Link to specific code only after checking the final file paths. Add a concrete implementation lesson in your own words if you want to expand this into a longer article.

## Product-update draft

> Vinny now has a clearer first-connection guide, troubleshooting steps, and guides for choosing a display and configuring a VNC viewer.
>
> If you need configurable VNC listeners on a Mac, try it and tell me where setup gets stuck: https://github.com/sarimabbas/vinny

Use this only once the documentation changes have merged and are visible publicly. Do not imply that an unreleased application change is available in the current release.

## Launch readiness

- [ ] Merge and deploy the reviewed documentation and website changes.
- [ ] Verify the advertised download exists and its architecture matches the page.
- [ ] Install the actual published archive or Homebrew cask on a Mac and complete the guide from a second machine.
- [ ] Validate each viewer recipe, permissions flow, selected-display behavior, view-only behavior, and troubleshooting guidance on supported macOS hardware.
- [ ] Record the real demo and add it to the README and website. A browser mockup is not evidence of a working connection.
- [ ] Invite ten willing testers and fix recurring blocking failures.
- [ ] Establish the first GitHub snapshot before posting.
- [ ] Review the drafts, add the founder's actual motivation where relevant, and check each community's current rules.
- [ ] Publish to one relevant audience first and record the link and time.
- [ ] Respond to reports, then launch to the next audience using what you learned.
- [ ] Review first connections after the launch and repeat usage seven days later.

The Mac validation, recording, tester recruitment, account setup, and public submissions require actual hardware, people, or account actions. They are not completed by preparing this repository kit.
