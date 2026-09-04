# App icon

Vinny ships an app icon even though it normally runs in the menu bar. macOS shows the icon in Finder, Spotlight, Open dialogs, System Settings, and permission prompts.

## Apple requirements

- Start with square, unmasked artwork. Do not draw the system's rounded-corner mask into the image. [App icons](https://developer.apple.com/design/human-interface-guidelines/app-icons)
- Keep important artwork away from the edges so masking does not crop it. [Icon shape](https://developer.apple.com/design/human-interface-guidelines/app-icons#Icon-shape)
- Use a simple image without text, screenshots, or copies of standard controls. [Design](https://developer.apple.com/design/human-interface-guidelines/app-icons#Design)
- Supply every macOS icon size in an asset catalog or equivalent container. [Configuring your app icon](https://developer.apple.com/documentation/xcode/configuring-your-app-icon-using-an-asset-catalog)

## Files and build

Vinny is built with Cargo rather than an Xcode app target. `assets/Vinny.icns` contains 16, 32, 128, 256, and 512 point images at 1x and 2x. The source artwork is `assets/vinny-app-icon-source.png`. The 1024-pixel master is `assets/vinny-app-icon.png`.

The artwork has these constraints:

- transparent pixels outside the portal
- Vinny facing forward, with the antenna and feet crossing the portal edge
- navy, cobalt, cyan, ivory, and coral colors
- no text or small interface details
- a clear silhouette at small sizes

The menu-bar icon is separate. It is a black-and-transparent AppKit template image with `isTemplate = true`, which lets macOS color it for the current appearance. See [`NSImage.isTemplate`](https://developer.apple.com/documentation/appkit/nsimage/istemplate).

If the project moves to an Xcode target, put the same artwork in an asset catalog or Icon Composer document instead of maintaining the ICNS file by hand.
