# Vinny app-icon requirements

Vinny should ship an application icon even though it runs as an `LSUIElement` menu-bar app and does not normally appear in the Dock. macOS still uses the application icon in Finder, Spotlight, Open dialogs, system settings, permission surfaces, and other app-identification contexts.

## Apple guidance

- Apple describes an app icon as the app's recognizable identity throughout the system and specifies a 1024×1024 square source for macOS. Supported color spaces include sRGB and Display P3. [App icons — Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/app-icons)
- Primary content should remain centered so system masking and presentation changes do not truncate it. Apple says to provide square, unmasked artwork rather than baking rounded corners into the source; pre-masking can produce jagged edges under system effects. [App icons — Icon shape](https://developer.apple.com/design/human-interface-guidelines/app-icons#Icon-shape)
- Apple recommends a simple, distinctive image rather than text, a screenshot, or a reproduction of standard interface controls. [App icons — Design](https://developer.apple.com/design/human-interface-guidelines/app-icons#Design)
- Xcode's preferred workflow is an app-icon asset catalog or Icon Composer. For macOS asset catalogs, Apple requires images for each supported size rather than relying on one automatically generated size. [Configuring your app icon using an asset catalog](https://developer.apple.com/documentation/xcode/configuring-your-app-icon)
- Icon Composer is the current layered workflow when an Xcode project needs the latest platform appearances and effects. [Creating your app icon using Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)

## Vinny implementation

Vinny's build is a Cargo-driven Developer ID distribution rather than an Xcode application target. It therefore packages a conventional `Vinny.icns` containing the complete macOS size matrix: 16, 32, 128, 256, and 512 points at 1× and 2×. Each raster is downsampled from the master with Lanczos filtering, then Apple's `iconutil` packages the matrix. `CFBundleIconFile` names that resource.

The original generated artwork is retained at `assets/vinny-app-icon-source.png`; the packaged 1024px master is `assets/vinny-app-icon.png`.

- transparent outside the authored rounded-square portal field
- Vinny faces forward and dominates the composition
- the antenna and feet overlap the portal edge to suggest Vinny climbing out
- restrained navy, cobalt, cyan, ivory, and coral accents
- recognizable cream monitor face, blue expression, and coral antenna
- no text or miniature interface details
- high contrast and a clear silhouette at small sizes

The colorful application icon and monochrome menu-bar icon are intentionally separate. The menu-bar image is a black-and-clear AppKit template image (`isTemplate = true`) so macOS can render it correctly in light, dark, highlighted, and accessibility appearances. Apple documents black-and-clear template imagery as the correct input for system-controlled image treatment. [NSImage.isTemplate](https://developer.apple.com/documentation/appkit/nsimage/istemplate)

If Vinny later adopts an Xcode target or Mac App Store distribution, migrate the same artwork into an asset catalog or Icon Composer document rather than hand-maintaining the ICNS container.
